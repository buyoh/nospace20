# Phase 1: ブロックスコープ変数の最小実装

## 目標

if/while ブロック内での `let:` を使用可能にし、変数のシャドウイングをサポートする。

## スコープ（対象範囲）

### 含む

- ブロック内での変数宣言 (`let:` in if/while)
- 変数のシャドウイング（同名変数によるマスク）
- ブロック終了時の変数破棄
- 子スコープから親スコープの変数へのアクセス

### 除外

- グローバル変数
- static 変数
- 識別子の事前解決（Phase 2）
- ネスト関数内からの親関数変数アクセス

## 前提条件

言語仕様（docs/spec.md セクション 7）より:

```nospace
func: main() {
  let:a;
  a = 1;
  if:1{
    let:b;
    a = 2;  # ok: 親スコープの変数にアクセス #
    b = 3;  # ok #
  };
  # b = 4;  # NG: 子スコープの変数にはアクセス不可 #
  # a == 2 (親スコープの変数が変更された) #
}
```

シャドウイングの例:

```nospace
func: main() {
  let:x;
  x = 1;
  if:1{
    let:x;  # シャドウイング #
    x = 3;  # このブロック内の x #
  };
  # x == 1 (外側の x は変更されていない) #
}
```

---

## 変更計画

### Step 1: Block 構造体の導入

**ファイル**: `src/semantic_analyzer/mod.rs`

現状の `ExecExpression::If` と `While` は `Vec<ExecStatement>` を直接保持している。
これを `Block` 構造体に変更し、スコープ情報を保持できるようにする。

#### 変更前

```rust
pub enum ExecExpression {
    If(Box<ExecExpression>, Vec<ExecStatement>, Vec<ExecStatement>),
    While(Box<ExecExpression>, Vec<ExecStatement>),
    ...
}
```

#### 変更後

```rust
/// ブロック（文の列とスコープ情報）
pub struct Block {
    pub scope: Scope,
    pub statements: Vec<ExecStatement>,
}

pub enum ExecExpression {
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    ...
}
```

### Step 2: ブロック内変数宣言の許可

**ファイル**: `src/semantic_analyzer/mod.rs`

`analyze_internal` 関数で、`ScopeType::Block` 時の変数宣言 panic を除去。

#### 変更前

```rust
Statement::VariableDeclaration(name, init) => {
    if let ScopeType::Block = scope_type {
        panic!("todo: block scoped variable is not implemented")
    }
    ...
}
```

#### 変更後

```rust
Statement::VariableDeclaration(name, init) => {
    if let ScopeType::Root = scope_type {
        panic!("todo: global variable is not implemented")
    }
    // ブロックスコープでも関数スコープでも変数宣言を許可
    scope.add_variable(name.clone(), Variable { identifier: name.clone() });
    ...
}
```

### Step 3: Block の構築方法

`analyze_internal` の戻り値は **現状のまま `(ScopeBuilder, Vec<ExecStatement>)` を維持**し、
呼び出し側で `Block` を構築する。

**理由**: 関数宣言時に引数を `ScopeBuilder` に追加する必要があるため、
`Block`（不変の `Scope` を含む）を返すと追加操作ができなくなる。

```rust
Statement::FunctionDeclaration(name, args, block) => {
    let (mut s, es) = analyze_internal(block, ScopeType::Function);
    // 引数を変数として追加（ScopeBuilder が必要）
    for a in args {
        s.add_variable(a.clone(), Variable { identifier: a.clone() });
    }
    // ここで Block を構築
    let block = Block {
        scope: s.build(),
        statements: es,
    };
    let func = Function {
        args: args.clone(),
        block,
    };
    scope.add_function(name.clone(), func);
}
```

#### If/While での Block 構築

If/While の内部ブロックは引数追加が不要なので、その場で `Block` を構築できる。

```rust
fn convert_to_exec_expression(expr: &Box<Expression>) -> Box<ExecExpression> {
    match expr.as_ref() {
        Expression::If(cond, stat1, stat2) => {
            let (s1, es1) = analyze_internal(stat1, ScopeType::Block);
            let (s2, es2) = analyze_internal(stat2, ScopeType::Block);
            Box::new(ExecExpression::If(
                convert_to_exec_expression(cond),
                Block { scope: s1.build(), statements: es1 },
                Block { scope: s2.build(), statements: es2 },
            ))
        }
        Expression::While(expr, stat) => {
            let (s, es) = analyze_internal(stat, ScopeType::Block);
            Box::new(ExecExpression::While(
                convert_to_exec_expression(expr),
                Block { scope: s.build(), statements: es },
            ))
        }
        ...
    }
}
```

#### まとめ

| 呼び出し箇所 | 戻り値の扱い |
|-------------|-------------|
| 関数宣言 | `ScopeBuilder` に引数追加後、`Block` 構築 |
| If/While | 即座に `Block` 構築 |
| ルートレベル | `Scope` のみ取り出し |

### Step 4: インタプリタにスコープスタック導入

**ファイル**: `src/interpreter/mod.rs`

`LocalEnvironment` を変更し、スコープスタックで変数を管理する。

#### 変更前

```rust
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    root_scope: &'a Scope,
    current_scope: &'a Scope,
    variables: BTreeMap<String, i64>,
}
```

#### 変更後

```rust
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    root_scope: &'a Scope,
    // スコープスタック: 末尾が現在のスコープ
    scope_stack: Vec<BTreeMap<String, i64>>,
}

impl LocalEnvironment<'_, '_> {
    /// ブロックに入る
    fn enter_block(&mut self, scope: &Scope) {
        let mut block_vars = BTreeMap::new();
        for v in scope.variables.iter() {
            block_vars.insert(v.identifier.clone(), 0);
        }
        self.scope_stack.push(block_vars);
    }

    /// ブロックから出る
    fn leave_block(&mut self) {
        self.scope_stack.pop();
    }

    /// 変数を取得（スコープスタックを上から探索）
    fn get_variable_mut(&mut self, name: &str) -> Option<&mut i64> {
        for scope in self.scope_stack.iter_mut().rev() {
            if let Some(val) = scope.get_mut(name) {
                return Some(val);
            }
        }
        None
    }
}
```

### Step 5: If/While の評価時にスコープ操作

`interpret_expression` 内の If/While 評価を変更。

#### 変更前

```rust
ExecExpression::If(cond, then_stmts, else_stmts) => {
    let c = try_expr!(self.interpret_expression(cond));
    if c != 0 {
        return self.interpret_statements(then_stmts);
    } else {
        return self.interpret_statements(else_stmts);
    }
}
```

#### 変更後

```rust
ExecExpression::If(cond, then_block, else_block) => {
    let c = try_expr!(self.interpret_expression(cond));
    if c != 0 {
        self.enter_block(&then_block.scope);
        let result = self.interpret_statements(&then_block.statements);
        self.leave_block();
        return result;
    } else {
        self.enter_block(&else_block.scope);
        let result = self.interpret_statements(&else_block.statements);
        self.leave_block();
        return result;
    }
}
```

---

## テストケース

### 既存テスト（修正後に通るべき）

- `scope_block_001.ns`: ブロック内での変数定義とシャドウイング
- `scope_func_001.ns`: 関数スコープの独立性
- `scope_nested_func_001.ns`: ネストした関数

### 追加すべきテスト

1. **シャドウイングなしの親変数アクセス**

```nospace
func: main() {
  let:x;
  x = 1;
  if:1{
    x = 2;  # 親の x を変更 #
  };
  __assert(x == 2);
}
```

2. **多重ネストブロック**

```nospace
func: main() {
  let:x;
  x = 1;
  if:1{
    let:y;
    y = 2;
    if:1{
      let:z;
      z = 3;
      __assert(x == 1);
      __assert(y == 2);
    };
    # z はアクセス不可 #
  };
  # y はアクセス不可 #
}
```

3. **while 内でのスコープ**

```nospace
func: main() {
  let:i;
  let:sum;
  i = 3;
  sum = 0;
  while:i{
    let:temp;
    temp = i;
    sum = sum + temp;
    i = i - 1;
  };
  __assert(sum == 6);
}
```

---

## 影響範囲

### 変更ファイル

1. `src/semantic_analyzer/mod.rs`
   - `Block` 構造体追加
   - `ExecExpression::If/While` の定義変更
   - `analyze_internal` の戻り値変更
   - ブロック変数宣言の panic 除去

2. `src/interpreter/mod.rs`
   - `LocalEnvironment` のスコープスタック化
   - `enter_block`/`leave_block` メソッド追加
   - `get_variable_mut` のスコープスタック探索
   - If/While 評価時のスコープ操作

### 変更しないファイル

- `src/tree_parser/` - 構文解析は変更不要
- `src/token_parser/` - トークン解析は変更不要
- `src/lib.rs` - インターフェースは維持

---

## 実装順序

1. **semantic_analyzer の Block 導入**（コンパイルエラー発生）
2. **interpreter の対応**（コンパイル通過）
3. **テスト実行・修正**

---

## リスク

1. **ライフタイムの複雑化**: Block が Scope を所有することでライフタイム管理が変わる
2. **既存テストの破壊**: If/While の構造が変わるため、全テストに影響
3. **パフォーマンス**: 実行時スコープ探索は O(depth) のコスト

## 成功基準

- `scope_block_001` テストが通る
- `scope_func_001` テストが通る
- `disabled_scope_block_var_001` を有効化してテストが通る
