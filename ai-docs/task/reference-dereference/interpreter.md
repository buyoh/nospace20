# interpreter モジュール変更設計

## 対象ファイル

- `src/interpreter/exec.rs`（主要変更）
- `src/interpreter/environment.rs`（アドレス解決補助）
- `src/interpreter/types.rs`（変更なし）

## 現状

### 値の表現

全ての値は `i64`。参照値（アドレス）も `i64` に埋め込む。

### 変数ストレージ

```rust
// exec.rs
struct LocalEnvironment<'a> {
    scope_stack: Vec<Vec<i64>>,   // ローカル変数
    env: &'a mut Environment,     // グローバル環境
}

// environment.rs
struct Environment {
    global_variables: Vec<i64>,   // グローバル変数（現在は Vec）
    // ...
}
```

### 変数アクセス

```rust
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    if id.is_global {
        self.env.global_variables[id.local_index]
    } else {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx][id.local_index]
    }
}
```

### global_variables の現状の問題

`global_variables` は現在 `Vec<i64>` だが、参照のアドレス解決を行う場合に内部実装に直接依存するとパフォーマンスチューニング時に修正が波及する。変数ストレージを抽象化し、実装を切り替え可能にすべき。

### Operation1 評価

```rust
fn interpret_operation1(&mut self, op: &Operator1, expr1: &Box<ExecExpression>) -> ExpressionFlow {
    let v1 = try_expr!(self.interpret_expression(expr1));
    let res = match op {
        Operator1::Negative => -v1,
        Operator1::LogicalNot => bool_to_int(v1 == 0),
    };
    ExpressionFlow::Value(res)
}
```

### 代入処理

```rust
if let Operator2::Assign = op {
    if let ExecExpression::Variable(id_ref) = expr1.as_ref() {
        let v = try_expr!(self.interpret_expression(expr2));
        self.set_variable(id_ref, v);
        return ExpressionFlow::Value(v);
    } else {
        panic!("runtime error: left value is not variable");
    }
}
```

## 変更内容

### 1. 変数ストレージの抽象化

`global_variables` は map 型（`BTreeMap<usize, i64>` 等）に変更してよい。ただしパフォーマンスの観点から、後から `Vec<i64>` やその他の実装に自由に切り替えられるよう、変数ストレージへのアクセスを `VariableStorage` トレイトで抽象化する。

```rust
/// 変数ストレージの抽象インターフェース
/// 
/// グローバル変数・ローカル変数のアクセス方法を統一し、
/// 内部実装（Vec, BTreeMap 等）を隠蔽する。
pub trait VariableStorage {
    fn get(&self, index: usize) -> i64;
    fn set(&mut self, index: usize, value: i64);
    fn len(&self) -> usize;
}
```

#### 初期実装: BTreeMap ベース

```rust
pub struct MapVariableStorage {
    data: BTreeMap<usize, i64>,
    len: usize,
}

impl VariableStorage for MapVariableStorage {
    fn get(&self, index: usize) -> i64 {
        *self.data.get(&index).unwrap_or(&0)
    }
    fn set(&mut self, index: usize, value: i64) {
        self.data.insert(index, value);
    }
    fn len(&self) -> usize {
        self.len
    }
}
```

#### 将来の最適化: Vec ベース

```rust
pub struct VecVariableStorage {
    data: Vec<i64>,
}

impl VariableStorage for VecVariableStorage {
    fn get(&self, index: usize) -> i64 {
        self.data[index]
    }
    fn set(&mut self, index: usize, value: i64) {
        self.data[index] = value;
    }
    fn len(&self) -> usize {
        self.data.len()
    }
}
```

#### Environment への適用

```rust
pub struct Environment {
    // 変更前: global_variables: Vec<i64>,
    // 変更後: トレイトオブジェクトまたはジェネリクスで抽象化
    pub(crate) global_variables: Box<dyn VariableStorage>,
    // ...
}
```

注意: トレイトオブジェクト (`Box<dyn VariableStorage>`) とジェネリクス (`Environment<S: VariableStorage>`) のどちらで実装するかは、呼び出し側への影響を考慮して判断する。トレイトオブジェクトのほうが既存コードへの影響が小さい。

#### ローカル変数への適用

ローカル変数 (`scope_stack: Vec<Vec<i64>>`) にも同トレイトを適用してよいが、ローカル変数は生存期間が短く頻繁にアクセスされるため、`Vec<i64>` のままでもよい。ローカル変数にまで抽象化を適用するかは実装時に判断する。

### 2. アドレス空間の設計

全変数をフラットなアドレス空間にマッピングする。
`VariableStorage::len()` を使い、内部実装に依存せずアドレスを計算する。

```
アドレス空間レイアウト:
  [0 .. global_count)                         → グローバル変数
  [global_count .. global_count + scope0_size) → スコープ0（最外側）のローカル変数
  [global_count + scope0_size .. ...)          → スコープ1のローカル変数
  ...

例:
  グローバル変数: g0, g1 (2個)
  スコープ0: a, b (2個)
  スコープ1: c (1個)

  g0 → address 0
  g1 → address 1
  a  → address 2
  b  → address 3
  c  → address 4
```

### 3. アドレス計算メソッド

アドレス計算は `VariableStorage` トレイトの `len()` を使い、内部実装に依存しない。

```rust
impl LocalEnvironment<'_, '_> {
    /// IdentifierRef から絶対アドレスを計算
    fn resolve_address(&self, id: &IdentifierRef) -> i64 {
        if id.is_global {
            id.local_index as i64
        } else {
            let global_count = self.env.global_variables.len() as i64;
            let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
            let mut addr = global_count;
            for i in 0..scope_idx {
                addr += self.scope_stack[i].len() as i64;
            }
            addr + id.local_index as i64
        }
    }

    /// 絶対アドレスから値を取得
    fn get_by_address(&self, addr: i64) -> i64 {
        let addr = addr as usize;
        let global_count = self.env.global_variables.len();
        if addr < global_count {
            self.env.global_variables.get(addr)
        } else {
            let mut remaining = addr - global_count;
            for scope in &self.scope_stack {
                if remaining < scope.len() {
                    return scope[remaining];
                }
                remaining -= scope.len();
            }
            panic!("runtime error: invalid address {}", addr);
        }
    }

    /// 絶対アドレスに値を設定
    fn set_by_address(&mut self, addr: i64, value: i64) {
        let addr = addr as usize;
        let global_count = self.env.global_variables.len();
        if addr < global_count {
            self.env.global_variables.set(addr, value);
        } else {
            let mut remaining = addr - global_count;
            for scope in &mut self.scope_stack {
                if remaining < scope.len() {
                    scope[remaining] = value;
                    return;
                }
                remaining -= scope.len();
            }
            panic!("runtime error: invalid address {}", addr);
        }
    }
}
```

### 4. Operation1 の拡張

```rust
fn interpret_operation1(&mut self, op: &Operator1, expr1: &Box<ExecExpression>) -> ExpressionFlow {
    match op {
        Operator1::Ref => {
            // & は Variable に対してのみ（意味解析で検証済み）
            if let ExecExpression::Variable(id_ref) = expr1.as_ref() {
                let addr = self.resolve_address(id_ref);
                ExpressionFlow::Value(addr)
            } else {
                panic!("runtime error: cannot take reference of non-variable");
            }
        }
        Operator1::Deref => {
            let addr = try_expr!(self.interpret_expression(expr1));
            let value = self.get_by_address(addr);
            ExpressionFlow::Value(value)
        }
        _ => {
            let v1 = try_expr!(self.interpret_expression(expr1));
            let res = match op {
                Operator1::Negative => -v1,
                Operator1::LogicalNot => bool_to_int(v1 == 0),
                _ => unreachable!(),
            };
            ExpressionFlow::Value(res)
        }
    }
}
```

### 5. 代入の左辺拡張

```rust
if let Operator2::Assign = op {
    match expr1.as_ref() {
        ExecExpression::Variable(id_ref) => {
            let v = try_expr!(self.interpret_expression(expr2));
            self.set_variable(id_ref, v);
            return ExpressionFlow::Value(v);
        }
        ExecExpression::Operation1(Operator1::Deref, inner) => {
            // *ptr = value のケース
            let addr = try_expr!(self.interpret_expression(inner));
            let v = try_expr!(self.interpret_expression(expr2));
            self.set_by_address(addr, v);
            return ExpressionFlow::Value(v);
        }
        _ => {
            panic!("runtime error: left value is not assignable");
        }
    }
}
```

## リスク・注意点

### ダングリングポインタ

スコープから抜けた変数のアドレスを保持している場合、そのアドレスは無効になるが、別のスコープの変数を指す可能性がある。C言語と同様に未定義動作として扱う。

### 再帰呼び出し

再帰関数の場合、同じ関数のローカル変数が複数のスコープスタックフレームに存在する。`resolve_address` は現在のスタック状態に基づいて計算するため、同一の `IdentifierRef` でも異なるアドレスを返す。これは正しい動作。

### パフォーマンス

`get_by_address` / `set_by_address` はスコープスタックを線形探索する（O(n)、nはスコープの深さ）。頻繁なデリファレンスがある場合のパフォーマンスへの影響に留意。最適化として、スコープごとのオフセットをキャッシュする方法がある。

`VariableStorage` トレイトにより、グローバル変数のストレージ実装を `BTreeMap` から `Vec` に切り替えたり、ローカル変数にも同トレイトを適用するなど、プロファイリング結果に基づく最適化が容易に行える。

## テスト

### ユニットテスト

```nospace
# 基本的な参照・デリファレンス
func: main() {
    let: x; let: p;
    x = 42;
    p = &x;
    __assert(*p == 42);
    return: 0;
}

# デリファレンス代入
func: main() {
    let: x; let: p;
    x = 10;
    p = &x;
    *p = 20;
    __assert(x == 20);
    return: 0;
}

# 関数引数としてのポインタ渡し
func: set_value(ptr, val) {
    *ptr = val;
}
func: main() {
    let: x;
    x = 0;
    set_value(&x, 100);
    __assert(x == 100);
    return: 0;
}
```
