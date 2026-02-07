# Phase 2: 識別子の事前解決

## 目標

意味解析時に変数名・関数名を解決し、実行時の文字列検索を排除することで、パフォーマンスを向上させる。

## スコープ（対象範囲）

### 含む

- 変数識別子の事前解決（`Variable(String)` → `Variable(IdentifierRef)`）
- 関数識別子の事前解決（`Function(String, ...)` → `Function(IdentifierRef, ...)`）
- 識別子参照の型定義（`IdentifierRef` 構造体）
- スコープ階層の記録（どのスコープの変数かを特定）

### 除外

- グローバル変数（Phase 3）
- static 変数（Phase 4）
- ネスト関数の可視性ルール（Phase 5）

## 現状分析

### Phase 1 完了後の状態

```rust
// 現在: 変数は文字列で保持
pub(crate) enum ExecExpression {
    Variable(String),           // 実行時に名前で検索
    Function(String, Vec<...>), // 実行時に名前で検索
    ...
}
```

```rust
// インタプリタ: 毎回スコープスタックを文字列検索
fn get_variable_mut(&mut self, name: &str) -> Option<&mut i64> {
    for scope in self.scope_stack.iter_mut().rev() {
        if let Some(val) = scope.get_mut(name) {  // O(log n) per scope
            return Some(val);
        }
    }
    None
}
```

### 問題点

1. **パフォーマンス**: 変数アクセスごとに O(depth × log n) の文字列検索
2. **メモリ**: 同じ変数名が複数箇所で String として複製される
3. **型安全性**: 存在しない変数への参照がコンパイル時に検出できない

---

## 設計

### 識別子参照の構造

```rust
/// 解決済み識別子への参照
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRef {
    /// スコープの深さ（0 = 現在のスコープ、1 = 親スコープ、...）
    pub scope_depth: usize,
    /// スコープ内でのインデックス
    pub local_index: usize,
}
```

この設計により:
- `scope_depth`: 何階層上のスコープかを示す
- `local_index`: そのスコープ内での変数のインデックス

### ExecExpression の変更

```rust
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    // 変更: String から IdentifierRef へ
    Function(IdentifierRef, Vec<Box<ExecExpression>>),
    Factor(i64),
    // 変更: String から IdentifierRef へ
    Variable(IdentifierRef),
}
```

### インタプリタの変更

```rust
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    root_scope: &'a Scope,
    // 変更: BTreeMap ではなく Vec<i64> を使用
    scope_stack: Vec<Vec<i64>>,
}

impl LocalEnvironment<'_, '_> {
    /// 識別子参照から値を取得
    fn get_variable(&self, id: &IdentifierRef) -> i64 {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx][id.local_index]
    }

    /// 識別子参照に値を設定
    fn set_variable(&mut self, id: &IdentifierRef, value: i64) {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx][id.local_index] = value;
    }

    fn enter_block(&mut self, scope: &Scope) {
        // 変数の数だけ初期化
        self.scope_stack.push(vec![0; scope.variables.len()]);
    }

    fn leave_block(&mut self) {
        self.scope_stack.pop();
    }
}
```

---

## 実装計画

### Step 1: IdentifierRef 構造体の追加

**ファイル**: `src/semantic_analyzer/mod.rs`

```rust
/// 解決済み識別子への参照
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRef {
    pub scope_depth: usize,
    pub local_index: usize,
}
```

### Step 2: スコープ解決コンテキストの導入

意味解析時にスコープスタックを保持し、識別子を解決する。

```rust
/// スコープ解決のためのコンテキスト
struct ScopeResolver<'a> {
    /// スコープスタック（末尾が現在のスコープ）
    /// 各スコープは変数名からローカルインデックスへのマップ
    scope_stack: Vec<&'a BTreeMap<String, usize>>,
}

impl<'a> ScopeResolver<'a> {
    fn new() -> Self {
        Self { scope_stack: Vec::new() }
    }

    fn enter_scope(&mut self, var_map: &'a BTreeMap<String, usize>) {
        self.scope_stack.push(var_map);
    }

    fn leave_scope(&mut self) {
        self.scope_stack.pop();
    }

    /// 変数名を解決し、IdentifierRef を返す
    fn resolve_variable(&self, name: &str) -> Option<IdentifierRef> {
        for (depth, scope) in self.scope_stack.iter().rev().enumerate() {
            if let Some(&local_index) = scope.get(name) {
                return Some(IdentifierRef { scope_depth: depth, local_index });
            }
        }
        None
    }
}
```

### Step 3: 2パス解析の導入

ホイスティング（巻き上げ）に対応するため、2パス方式で解析する。

#### 現状の問題

```nospace
func: main() {
  a = 5;    # 使用 (先) #
  let: a;   # 定義 (後) - ホイスティングにより有効 #
}
```

1パスでは `a = 5` の時点で `a` が未定義のため解決できない。

#### 2パス方式

**パス1: 宣言収集**
- スコープ内の全ての `let:` と `func:` を収集
- 各スコープの変数テーブルを構築

**パス2: 識別子解決**
- 変数参照・関数呼び出しを `IdentifierRef` に変換
- 構築済みの変数テーブルを参照

#### 実装アプローチ

```rust
fn analyze_internal(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    let mut scope = ScopeBuilder::new(scope_type);
    
    // パス1: 宣言収集
    for stmt in statements {
        match stmt.statement {
            Statement::VariableDeclaration(ref name, _) => {
                scope.add_variable(name.clone(), Variable { identifier: name.clone() });
            }
            Statement::FunctionDeclaration(ref name, _, _) => {
                // 関数もパス1で登録（ホイスティング）
                // ただし本体の解析はパス2で行う
            }
            _ => {}
        }
    }

    // パス2: 文の変換（識別子解決を伴う）
    let resolver = ScopeResolver::new_with_scope(&scope);
    let exec_stmts = convert_statements_with_resolver(statements, &resolver)?;

    Ok((scope, exec_stmts))
}
```

### Step 4: Scope 構造の変更

変数のローカルインデックスを保持するように変更。

```rust
pub struct Scope {
    /// 変数名からローカルインデックスへのマップ
    variable_indices: BTreeMap<String, usize>,
    /// 変数の数
    pub variable_count: usize,
    
    /// 関数名から関数定義へのマップ
    functions: BTreeMap<String, Function>,
}
```

### Step 5: インタプリタの Vec 化

`BTreeMap<String, i64>` から `Vec<i64>` に変更。

```rust
impl LocalEnvironment<'_, '_> {
    fn enter_block(&mut self, scope: &Scope) {
        self.scope_stack.push(vec![0; scope.variable_count]);
    }

    fn interpret_expression(&mut self, expr: &ExecExpression) -> Result<i64, ...> {
        match expr {
            ExecExpression::Variable(id_ref) => {
                let scope_idx = self.scope_stack.len() - 1 - id_ref.scope_depth;
                Ok(self.scope_stack[scope_idx][id_ref.local_index])
            }
            ...
        }
    }
}
```

---

## 移行戦略

### 段階的移行

1. **Step A**: 新旧両方の形式をサポート
   ```rust
   pub(crate) enum ExecExpression {
       Variable(String),              // 旧: 文字列
       VariableResolved(IdentifierRef), // 新: 解決済み
       ...
   }
   ```

2. **Step B**: テストを通しながら convert_to_exec_expression を移行

3. **Step C**: 旧形式を削除

### または一括移行

Phase 1 と同様に、一度に全てを変更してテストで確認する方法もある。
コードベースが小さいため、一括移行の方が効率的かもしれない。

---

## テストケース

### 既存テストの動作確認

Phase 1 で追加されたスコープ関連テストが引き続き通ることを確認:
- `scope_block_001.ns`
- `scope_func_001.ns`
- シャドウイング関連テスト

### 追加すべきテスト

1. **ホイスティングの動作確認**

```nospace
func: main() {
  x = 5;     # 定義より前に使用 #
  let: x;
  __assert(x == 5);
}
```

2. **深いネストでの変数解決**

```nospace
func: main() {
  let:a;
  a = 1;
  if:1{
    if:1{
      if:1{
        __assert(a == 1);  # 3階層上の変数 #
        a = 2;
      };
    };
  };
  __assert(a == 2);
}
```

3. **同名変数のシャドウイングと解決**

```nospace
func: main() {
  let:x;
  x = 1;
  if:1{
    let:x;
    x = 2;
    if:1{
      let:x;
      x = 3;
      __assert(x == 3);  # 最も近い x #
    };
    __assert(x == 2);
  };
  __assert(x == 1);
}
```

---

## 影響範囲

### 変更ファイル

1. **`src/semantic_analyzer/mod.rs`**
   - `IdentifierRef` 構造体追加
   - `ExecExpression::Variable/Function` の型変更
   - `ScopeResolver` 導入
   - 2パス解析の実装

2. **`src/interpreter/mod.rs`**
   - `LocalEnvironment` の `scope_stack` を `Vec<Vec<i64>>` に変更
   - `get_variable`/`set_variable` の引数を `IdentifierRef` に変更
   - 代入式の処理変更

### 変更しないファイル

- `src/tree_parser/` - 構文解析は変更不要
- `src/token_parser/` - トークン解析は変更不要

---

## リスク

1. **2パス解析の複雑さ**
   - 中間表現が必要になる可能性
   - ライフタイム管理が複雑化する可能性

2. **ホイスティングの正確な実装**
   - 初期値の評価タイミングに注意
   - 関数のホイスティングとの整合性

3. **テストの更新**
   - ExecExpression の構造変更によりユニットテストの更新が必要

---

## パフォーマンス期待効果

| 操作 | Phase 1 (現在) | Phase 2 (実装後) |
|------|---------------|-----------------|
| 変数読み取り | O(depth × log n) | O(1) |
| 変数書き込み | O(depth × log n) | O(1) |
| メモリ | 変数名を複製 | インデックスのみ |

---

## 成功基準

- 全既存テストが通過
- 変数アクセスが O(1) になる
- ExecExpression に文字列の Variable/Function が残らない
- ホイスティングが正しく動作する

---

## 参考: 過去の実装（7a83612）

過去の実装では以下の構造を使用していた:

```rust
pub struct Identifier {
    pub scope: usize,
    pub local: usize,
}
```

この設計は `IdentifierRef` と類似しており、参考になる。
ただし過去の実装は `PendingScope`/`PendingBlock` など複雑な中間構造を
導入していたため revert された。

Phase 2 では、よりシンプルな実装を目指す。
