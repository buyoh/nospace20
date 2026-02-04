# Phase 3: グローバル変数

## 目標

関数外での変数宣言（グローバル変数）をサポートする。

## 言語仕様 (spec.md セクション 7)

- グローバルスコープは関数スコープと同様
- 親スコープから子スコープの変数にはアクセス不可
- 子スコープから親スコープの変数にアクセス可能

```nospace
let: global_counter;
global_counter = 0;

func: increment() {
  global_counter = global_counter + 1;  # グローバル変数にアクセス #
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
- 関数からグローバル変数へのアクセス
- グローバル変数の初期化式

### 除外

- static 変数（Phase 4）
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

### アプローチ: グローバルスコープを最初のスコープとして追加

グローバル変数を特別扱いせず、スコープスタックの最下層として扱う。

### 変更点

#### 1. Scope 構造にグローバル変数を追加

ルートスコープ（`analyze` の戻り値）に変数情報を保持。

```rust
pub struct Scope {
    identifier_map: BTreeMap<String, Identifier>,
    variable_indices: BTreeMap<String, usize>,
    pub(crate) variables: Vec<Variable>,
    pub(crate) variable_count: usize,
    functions: Vec<Function>,
}
```

現状の `Scope` 構造で十分。`analyze_internal` でルートスコープにも変数を追加できるように変更するのみ。

#### 2. semantic_analyzer の変更

ルートスコープでの変数宣言を許可:

```rust
Statement::VariableDeclaration(name, _) => {
    // グローバル変数も通常の変数と同様に扱う
    scope.add_variable(
        name,
        Variable {
            identifier: name.clone(),
        },
    )?;
}
```

#### 3. ScopeResolver にグローバルスコープを追加

関数本体を解析する際、グローバルスコープの変数も参照できるようにする:

```rust
fn analyze_internal_with_parent(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    initial_vars: Vec<String>,
    parent_resolver: Option<&ScopeResolver>,  // グローバルスコープを含む
) -> Result<...>
```

ただし、仕様では「親の関数スコープにはアクセスできない」とあるため、
**グローバルスコープはブロックスコープと同様に扱う**のが正しい。

#### 4. インタプリタの変更

##### 4.1 グローバル変数の値保持

`Scope` に加えて、グローバル変数の値を保持する領域が必要:

```rust
pub fn interpret_func(
    env: &mut Environment,
    scope: &Scope,
    func_name: &str,
    global_vars: &mut Vec<i64>,  // グローバル変数の値
) -> Option<i64>
```

または、`Environment` にグローバル変数を追加:

```rust
pub struct Environment {
    pub traced: BTreeMap<i64, i64>,
    pub(crate) stdin: Box<dyn BufRead>,
    pub(crate) stdout: Box<dyn Write>,
    pub config: EnvironmentConfig,
    metrics: EnvironmentMetrics,
    /// Phase 3: グローバル変数の値
    pub global_variables: Vec<i64>,
}
```

##### 4.2 LocalEnvironment の変更

```rust
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    root_scope: &'a Scope,
    /// グローバル変数の値への参照
    /// scope_stack の最下層としてアクセス
    global_vars: &'a mut Vec<i64>,
    /// 関数・ブロックスコープスタック
    scope_stack: Vec<Vec<i64>>,
}
```

##### 4.3 変数アクセスの変更

```rust
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    let total_depth = self.scope_stack.len();
    if id.scope_depth >= total_depth {
        // グローバル変数にアクセス
        self.global_vars[id.local_index]
    } else {
        let scope_idx = total_depth - 1 - id.scope_depth;
        self.scope_stack[scope_idx][id.local_index]
    }
}
```

---

## 詳細設計

### IdentifierRef の scope_depth について

Phase 2 で導入した `IdentifierRef` の `scope_depth` は、現在のスコープからの相対深度を表す:

- 0: 現在のスコープ
- 1: 親スコープ
- 2: 祖父スコープ
- ...

グローバル変数は最も深い親スコープとして扱われる。

### 問題: グローバル変数と関数スコープ変数の区別

仕様では：
- 「親の**関数スコープ**にはアクセスできない」
- 「グローバルスコープは関数スコープと同様」

これは、関数からグローバル変数にはアクセス**できない**という意味か？

#### 仕様の解釈

```nospace
func: fn1() {
  let: x1;
  func: fn2() {
    let: x2;
    # x1 = 1;  Bad: 親の関数スコープにアクセス不可 #
  }
}
```

これは**ネスト関数**の場合。

グローバル変数は**ネスト関数ではない**ため、別のルールが適用される可能性がある。

#### 設計決定: グローバル変数はアクセス可能とする

多くの言語でグローバル変数は関数からアクセス可能なため、
nospace でもグローバル変数は関数からアクセス可能とする。

「親の関数スコープにはアクセスできない」はネスト関数に対するルールと解釈。

---

## 実装計画

### Step 1: semantic_analyzer の変更

1. ルートスコープでの変数宣言を許可
2. 関数宣言時、関数本体の resolver にルートスコープを親として渡す

### Step 2: インタプリタの変更

1. `Environment` に `global_variables: Vec<i64>` を追加
2. `interpret_func` 呼び出し前にグローバル変数を初期化
3. `LocalEnvironment` でグローバル変数スコープを参照
4. `get_variable` / `set_variable` でグローバル変数にアクセス

### Step 3: ルートスコープの初期化式実行

グローバル変数の初期化式を実行するタイミング:
- `main()` 呼び出し前に実行

```rust
pub fn interpret(env: &mut Environment, scope: &Scope) -> Option<i64> {
    // グローバル変数の初期化
    let mut global_vars = vec![0; scope.variable_count];
    // 初期化式を実行
    for stmt in &scope.root_statements {
        // ...
    }
    
    // main() を呼び出し
    interpret_func(env, scope, "main", &mut global_vars)
}
```

### Step 4: テストケースの追加

- `scope_global_001.ns`: 基本的なグローバル変数
- `scope_global_shadow_001.ns`: グローバル変数のシャドウイング
- `scope_global_init_001.ns`: グローバル変数の初期化式

---

## 移行戦略

### 段階的実装

1. **Step A**: グローバル変数の宣言を許可（意味解析）
2. **Step B**: グローバル変数を関数から参照可能にする（識別子解決）
3. **Step C**: インタプリタでグローバル変数を実行（値の保持と初期化）
4. **Step D**: テストケース追加

---

## 影響範囲

### 変更ファイル

1. **`src/semantic_analyzer/mod.rs`**
   - ルートスコープでの変数宣言を許可
   - 関数の resolver にルートスコープを追加

2. **`src/interpreter/mod.rs`**
   - `Environment` に `global_variables` 追加
   - `LocalEnvironment` の変数アクセスを拡張
   - ルート初期化式の実行ロジック

3. **`src/lib.rs`**（必要に応じて）
   - エントリーポイントの変更

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

1. **初期化順序の複雑さ**
   - グローバル変数の初期化式が相互依存する場合
   - 解決: 宣言順に初期化

2. **scope_depth の計算**
   - グローバルスコープを含めた深度計算
   - 解決: 関数スコープの開始時に +1 を考慮

3. **テストの更新**
   - 既存テストへの影響は少ないはず

---

## 成功基準

- 全既存テストが通過
- グローバル変数の宣言・代入・参照が動作
- 関数内からグローバル変数にアクセス可能
- グローバル変数のシャドウイングが動作
- 初期化式が正しく実行される

---

## 参考

### Phase 2 からの継続

Phase 2 で導入した以下の仕組みを拡張:
- `IdentifierRef` によるインデックスベースのアクセス
- `ScopeResolver` による親スコープの探索
- `Vec<i64>` ベースの変数ストレージ

### 関連ドキュメント

- [overview.md](overview.md) - スコープ実装の全体像
- [../../done-task/scope-phase2-identifier-resolution.md](../../done-task/scope-phase2-identifier-resolution.md) - Phase 2 完了レポート
