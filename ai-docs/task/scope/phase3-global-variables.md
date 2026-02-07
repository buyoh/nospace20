# Phase 3: グローバル変数

## 目標

関数外での変数宣言（グローバル変数）をサポートする。

## 言語仕様 (spec.md セクション 7)

- グローバルスコープは関数スコープと同様
- 親スコープから子スコープの変数にはアクセス不可
- 子スコープから親スコープの変数にアクセス可能
- **親の関数スコープにはアクセスできない**（変数の場合）
- **関数は static な定数として定義される**ため、親の関数スコープの関数にはアクセス可能
- static 変数は親の関数スコープにアクセス可能

### 仕様解釈

グローバル変数について：
- グローバルスコープは「関数スコープと同様」
- したがって、関数内からグローバル変数にアクセスするには static が必要
- **グローバル変数は暗黙的に static として扱う**（関数が暗黙的に static なのと同様）

```nospace
let: global_counter;    # 暗黙的に static #
global_counter = 0;

func: increment() {
  global_counter = global_counter + 1;  # static なのでアクセス可能 #
}

func: main() {
  increment();
  increment();
  __assert(global_counter == 2);
}
```

## スコープ（対象範囲）

### 含む

- ルートスコープでの変数宣言 (`let:` at root level)
- グローバル変数への暗黙的 static フラグ付与
- 関数からグローバル変数へのアクセス（static 経由）
- グローバル変数の初期化式
- Variable 構造体への `is_static` フラグ追加（Phase 4 の基盤）

### 除外

- ローカル変数の明示的 static 宣言（Phase 4）
- ネスト関数の可視性ルール（Phase 5）

---

## 現状分析

### semantic_analyzer/mod.rs

```rust
Statement::VariableDeclaration(name, _) => {
    if let ScopeType::Root = scope_type {
        // TODO(unimplemented): グローバル変数は未実装
        return Err(vec![code_parse_error!(
            located_stat.location.start,
            "semantic error: global variable is not implemented".to_string()
        )]);
    }
    // ...
}
```

### interpreter/mod.rs

現在、インタプリタはルートスコープを `root_scope` として保持しているが、
グローバル変数の値を保持する仕組みがない。

```rust
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    root_scope: &'a Scope,       // 関数定義のみ
    scope_stack: Vec<Vec<i64>>,  // 関数・ブロック内の変数のみ
}
```

---

## 設計

### アプローチ: static フラグによるスコープ境界の制御

1. Variable 構造体に `is_static` フラグを追加
2. グローバル変数は暗黙的に `is_static = true`
3. 識別子解決時、関数スコープ境界を越える場合は static 変数のみ参照可能
4. グローバル変数の値は `Environment` で保持

### 変更点

#### 1. Variable 構造体に is_static フラグを追加

```rust
pub(crate) struct Variable {
    pub identifier: String,
    /// Phase 3: static フラグ
    /// true の場合、親の関数スコープからもアクセス可能
    pub is_static: bool,
}
```

#### 2. Scope 構造に関数スコープ境界フラグを追加

```rust
pub struct Scope {
    identifier_map: BTreeMap<String, Identifier>,
    variable_indices: BTreeMap<String, usize>,
    pub(crate) variables: Vec<Variable>,
    pub(crate) variable_count: usize,
    functions: Vec<Function>,
    /// Phase 3: このスコープが関数スコープかどうか
    /// true の場合、非 static 変数は親スコープからアクセス不可
    pub(crate) is_function_scope: bool,
}
```

#### 3. ScopeResolver の変更

識別子解決時に関数スコープ境界をチェック：

```rust
fn resolve_variable(&self, name: &str) -> Option<IdentifierRef> {
    let mut crossed_function_boundary = false;
    
    for (depth, (scope_info, var_map)) in self.scope_stack.iter().rev().enumerate() {
        if scope_info.is_function_scope && depth > 0 {
            crossed_function_boundary = true;
        }
        
        if let Some(&local_index) = var_map.get(name) {
            // 関数境界を越えた場合、static 変数のみアクセス可能
            if crossed_function_boundary && !scope_info.variables[local_index].is_static {
                continue;  // 非 static 変数はスキップ
            }
            return Some(IdentifierRef { scope_depth: depth, local_index });
        }
    }
    None
}
```

#### 4. semantic_analyzer の変更

ルートスコープでの変数宣言を許可し、暗黙的に static を付与：

```rust
Statement::VariableDeclaration(name, _) => {
    let is_static = matches!(scope_type, ScopeType::Root);
    scope.add_variable(
        name,
        Variable {
            identifier: name.clone(),
            is_static,
        },
    )?;
}
```

#### 5. インタプリタの変更

##### 5.1 Environment にグローバル変数を追加

```rust
pub struct Environment {
    pub traced: BTreeMap<i64, i64>,
    pub(crate) stdin: Box<dyn BufRead>,
    pub(crate) stdout: Box<dyn Write>,
    pub config: EnvironmentConfig,
    metrics: EnvironmentMetrics,
    /// Phase 3: グローバル変数の値
    pub(crate) global_variables: Vec<i64>,
}
```

##### 5.2 IdentifierRef にグローバルフラグを追加

```rust
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRef {
    pub scope_depth: usize,
    pub local_index: usize,
    /// Phase 3: グローバル変数かどうか
    pub is_global: bool,
}
```

##### 5.3 変数アクセスの変更

```rust
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    if id.is_global {
        self.env.global_variables[id.local_index]
    } else {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx][id.local_index]
    }
}

fn set_variable(&mut self, id: &IdentifierRef, value: i64) {
    if id.is_global {
        self.env.global_variables[id.local_index] = value;
    } else {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx][id.local_index] = value;
    }
}
```

---

## 詳細設計

### スコープ階層と static の関係

```
グローバルスコープ（関数スコープ相当）
  └─ let: global_x (is_static = true, 暗黙的)
  └─ func: main()
       └─ 関数スコープ
            └─ let: local_x (is_static = false)
            └─ if: { ブロックスコープ
                 └─ let: block_x (is_static = false)
               }
```

### 変数アクセスのルール

| 変数の位置 | アクセス元 | is_static | アクセス可否 |
|-----------|-----------|-----------|------------|
| グローバル | main() 内 | true（暗黙） | ✅ 可能 |
| main() 内 | ネスト関数内 | false | ❌ 不可 |
| main() 内 | main() 内ブロック | - | ✅ 可能（ブロックは関数境界でない） |

### IdentifierRef の is_global について

`is_global` を追加する理由：
- グローバル変数は `Environment` に保持される
- ローカル変数は `LocalEnvironment.scope_stack` に保持される
- アクセス先を区別する必要がある

代替案: `scope_depth` に特別な値（例: `usize::MAX`）を使用してグローバルを示す
→ 可読性のため `is_global` フラグを採用

---

## 実装計画

### Step 1: Variable 構造体の拡張

1. `is_static` フラグを追加
2. 既存の変数宣言は `is_static = false` で初期化

### Step 2: Scope 構造の拡張

1. `is_function_scope` フラグを追加
2. ScopeBuilder で関数スコープかどうかを記録

### Step 3: semantic_analyzer の変更

1. ルートスコープでの変数宣言を許可
2. グローバル変数に `is_static = true` を付与
3. 関数本体の resolver にルートスコープを親として渡す
4. 識別子解決時に関数境界と static をチェック

### Step 4: IdentifierRef の拡張

1. `is_global` フラグを追加
2. グローバル変数解決時に `is_global = true` を設定

### Step 5: インタプリタの変更

1. `Environment` に `global_variables: Vec<i64>` を追加
2. `get_variable` / `set_variable` で `is_global` をチェック
3. ルート初期化式の実行ロジックを追加

### Step 6: テストケースの追加

- `scope_global_001.ns`: 基本的なグローバル変数
- `scope_global_shadow_001.ns`: グローバル変数のシャドウイング
- `scope_global_init_001.ns`: グローバル変数の初期化式

---

## Phase 4 との関係

Phase 4 では以下を実装予定：
- ローカル変数への明示的 `static` 修飾子
- ネスト関数から親関数スコープの static 変数へのアクセス

Phase 3 で導入する `is_static` フラグと関数境界チェックの仕組みは、
Phase 4 でそのまま活用される。

```nospace
# Phase 4 で対応予定 #
func: outer() {
  static let: counter;  # 明示的 static #
  counter = 0;
  
  func: inner() {
    counter = counter + 1;  # static なのでアクセス可能 #
  }
}
```

---

## 影響範囲

### 変更ファイル

1. **`src/semantic_analyzer/mod.rs`**
   - Variable に `is_static` 追加
   - Scope に `is_function_scope` 追加
   - ルートスコープでの変数宣言を許可
   - 識別子解決で関数境界と static をチェック
   - IdentifierRef に `is_global` 追加

2. **`src/interpreter/mod.rs`**
   - `Environment` に `global_variables` 追加
   - 変数アクセスで `is_global` をチェック
   - ルート初期化式の実行

### 変更しないファイル

- `src/tree_parser/` - 構文解析は変更不要
- `src/token_parser/` - トークン解析は変更不要

---

## テストケース

### 基本的なグローバル変数

```nospace
# scope_global_001.ns #
let: counter;
counter = 0;

func: inc() {
  counter = counter + 1;
}

func: main() {
  __assert(counter == 0);
  inc();
  __assert(counter == 1);
  inc();
  __assert(counter == 2);
}
```

### グローバル変数のシャドウイング

```nospace
# scope_global_shadow_001.ns #
let: x;
x = 10;

func: main() {
  __assert(x == 10);  # グローバル変数 #
  let: x;
  x = 20;
  __assert(x == 20);  # ローカル変数（シャドウイング） #
}
```

### グローバル変数の初期化

```nospace
# scope_global_init_001.ns #
let: a;
let: b;
a = 5;
b = a + 3;  # 初期化式で他のグローバル変数を参照 #

func: main() {
  __assert(a == 5);
  __assert(b == 8);
}
```

---

## リスク

1. **ScopeResolver の複雑化**
   - 関数境界チェックと static チェックの追加
   - 解決: 明確なコメントとテストで対応

2. **Phase 4 との整合性**
   - Phase 4 で static 変数を追加する際の互換性
   - 解決: is_static フラグを汎用的に設計

3. **既存テストへの影響**
   - Variable 構造体の変更による影響
   - 解決: デフォルト値 `is_static = false` で後方互換性を維持

---

## 成功基準

- 全既存テストが通過
- グローバル変数の宣言・代入・参照が動作
- 関数内からグローバル変数にアクセス可能（暗黙的 static により）
- グローバル変数のシャドウイングが動作
- 初期化式が正しく実行される
- Phase 4 の static 実装に向けた基盤が整備される

---

## 参考

### Phase 2 からの継続

Phase 2 で導入した以下の仕組みを拡張:
- `IdentifierRef` によるインデックスベースのアクセス → `is_global` 追加
- `ScopeResolver` による親スコープの探索 → 関数境界チェック追加
- `Vec<i64>` ベースの変数ストレージ → グローバル変数用ストレージ追加

### 関連ドキュメント

- [overview.md](overview.md) - スコープ実装の全体像
- [../../done-task/scope-phase2-identifier-resolution.md](../../done-task/scope-phase2-identifier-resolution.md) - Phase 2 完了レポート
