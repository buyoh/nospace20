# Phase 5 ネスト関数実装 - スタックオーバーフロー調査

## 問題の概要

ネスト関数を含むテストを実行すると、スタックオーバーフローが発生する。

## 再現手順

```bash
cargo run --bin nospace20 -- resources/tests/passes/scope/scope_nested_func_001.ns
```

または

```bash
cargo run --bin nospace20 -- tmp/test-nested-actual.ns
```

## エラーメッセージ

```
thread 'main' (32301604) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

## テストケース

### 成功するケース（ネスト関数なし）

```nospace
# tmp/test-nested-simple.ns
func: outer() {
  __trace(1);
}

func: main() {
  __trace(0);
  outer();
}
```

結果: 成功

### 失敗するケース（ネスト関数あり）

```nospace
# tmp/test-nested-actual.ns
func: outer() {
  __trace(1);

  func: inner() {
    __trace(2);
  }

  inner();
}

func: main() {
  __trace(0);
  outer();
}
```

結果: スタックオーバーフロー

## 根本原因（確定）

### インタプリタの関数ルックアップが常に `root_scope` を参照する

**発生箇所**: `src/interpreter/exec.rs` の `interpret_call_user_function_by_ref` メソッド（L259-335）

**問題のコード**:

```rust
let func = if func_ref.is_global {
    &self.root_scope.functions[func_ref.local_index]
} else {
    // ネスト関数: スコープスタックから取得
    // ...
    // Phase 5 では、全ての関数は root_scope に登録されている想定
    // （ネスト関数もグローバルに登録される）
    &self.root_scope.functions[func_ref.local_index]
};
```

`is_global` が `true` でも `false` でも、**常に `self.root_scope.functions` にアクセスしている**。
ネスト関数は `root_scope.functions` ではなく、親関数の `block.scope.functions` に格納されているため、
**誤った関数が呼び出される**。

### 実行フローのトレース

1. **意味解析時（semantic analyzer）**:
   - ルートスコープのパス1a: `outer`(idx=0), `main`(idx=1) を `root_scope.functions` に登録
   - `outer` 本体のパス1a: `inner`(idx=0) を `outer.block.scope.functions` に登録
   - resolver が `inner()` の呼び出しを解決: `IdentifierRef { scope_depth: 0, local_index: 0, is_global: false }`

2. **実行時（interpreter）**:
   - `main()` → `outer()` を呼び出し（`is_global=true`, `local_index=0` → `root_scope.functions[0]` = `outer` ✓）
   - `outer()` 本体 → `inner()` を呼び出し（`is_global=false`, `local_index=0`）
   - `interpret_call_user_function_by_ref`: `root_scope.functions[0]` = **`outer`** を取得（`inner` ではない！）
   - `outer()` が再度実行される → `inner()` → `outer()` → `inner()` → ... **無限再帰**
   - スタックオーバーフロー発生

### なぜ無限再帰になるか

- `inner` のインデックス `0` は `outer.block.scope.functions` 内のインデックス
- `root_scope.functions[0]` は `outer` 関数自体
- したがって `inner()` 呼び出し → `outer()` 実行 → `inner()` 呼び出し → `outer()` 実行 → ...

## 根本的なアーキテクチャ上の問題

### 問題1: インタプリタの `scope_stack` が関数定義を保持していない

`LocalEnvironment.scope_stack` は `Vec<Vec<i64>>` であり、変数の値のみを保持している。
スコープのメタデータ（特に関数定義）にはアクセスできない。

```rust
pub(super) struct LocalEnvironment<'a, 'aenv> {
    pub(super) env: &'aenv mut Environment,
    pub(super) root_scope: &'a Scope,
    pub(super) scope_stack: Vec<Vec<i64>>,  // 値のみ。Scope 情報なし
}
```

### 問題2: ネスト関数の格納場所

ネスト関数は親関数の `Block.scope.functions` に格納される（意味解析で正しく処理されている）。
しかし、インタプリタは `root_scope.functions` しかアクセスできないため、ネスト関数を見つけられない。

### 問題3: `initialize_function_statics` もネスト関数を処理しない

`src/interpreter/mod.rs` の `initialize_function_statics` はルートスコープの関数のみを走査する。
ネスト関数内の static 変数の初期化は行われない。

## 修正方針: 関数をフラット化してルートスコープに全登録（方針B）

### なぜ方針Bが適切か

nospace の関数は**すべて static**（定義時の変数をキャプチャしない）。
これは変数の視点で言えば、ネスト関数から親の非 static 変数にはアクセスできない（`static` 宣言された変数のみアクセス可能）ことを意味する。

つまり、関数はどのスコープで定義されていても、**実行時に宣言位置のコンテキスト（ローカル変数フレーム）を必要としない**。
したがって、全関数をルートスコープにフラットに格納しても、実行時の動作に影響しない。

変数の場合はスコープごとに別の値を持つためスコープスタックが必要だが、
関数定義は不変かつ静的であるため、グローバルな配列に全て格納するのが自然である。

### 方針Bの核心: 「格納はフラット、可視性はスコープ」

**変数の識別子解決との対比**:

| | 変数 | 関数（方針B） |
|---|---|---|
| **格納場所** | スコープごとの `Vec<i64>` | ルートスコープの `functions: Vec<Function>`（フラット） |
| **名前解決** | 各スコープの `identifier_map` を探索 | 各スコープの `identifier_map` を探索（同じ） |
| **実行時参照** | `scope_stack[len-1-depth][local_index]` | `root_scope.functions[local_index]` |
| **`IdentifierRef.is_global`** | スコープ位置に依存 | 常に `true`（実質不要） |
| **`IdentifierRef.scope_depth`** | 必要（スコープスタックの位置特定） | 不要（常にルートスコープ） |

ポイント: **名前→インデックスの解決は変数と同じ仕組み**（スコープごとの `identifier_map`）を使うが、
**インデックスが指す先はグローバルな関数リスト**という点が異なる。

### 実行フローのトレース（修正後）

テストケース:
```nospace
func: outer() {
  __trace(1);
  func: inner() { __trace(2); }
  inner();
}
func: main() {
  __trace(0);
  outer();
}
```

**意味解析時**:

1. ルートスコープのパス1a:
   - `outer` → `global_functions[0]`、ルートの `identifier_map["outer"] = Function(idx: 0)`
   - `main` → `global_functions[1]`、ルートの `identifier_map["main"] = Function(idx: 1)`

2. `outer` 本体のパス1a:
   - `inner` → `global_functions[2]`、outer スコープの `identifier_map["inner"] = Function(idx: 2)`
   - ※ `idx: 2` はグローバルインデックス（ローカルインデックスではない）

3. `inner()` 呼び出しの解決:
   - `resolve_function("inner")` → outer スコープの `func_map` で発見 → `IdentifierRef { local_index: 2, is_global: true }`

**実行時**:

1. `main()` → `outer()` 呼び出し: `root_scope.functions[0]` = `outer` ✓
2. `outer()` 本体 → `inner()` 呼び出し: `root_scope.functions[2]` = `inner` ✓
3. 無限再帰なし ✓

### エッジケースの検証

#### 1. 同名関数が異なるスコープに存在

```nospace
func: foo() {
  func: helper() { __trace(10); }
  helper();
}
func: bar() {
  func: helper() { __trace(20); }
  helper();
}
```

- `foo::helper` → `global_functions[1]`、foo の `identifier_map["helper"] = Function(idx: 1)`
- `bar::helper` → `global_functions[3]`、bar の `identifier_map["helper"] = Function(idx: 3)`
- 各スコープの `identifier_map` が異なるインデックスを指すため正しく動作 ✓

#### 2. 深いネスト

```nospace
func: a() {
  func: b() {
    func: c() { __trace(1); }
    c();
  }
  b();
}
```

- `a` → `global_functions[0]`
- `b` → `global_functions[1]`（a のスコープで可視）
- `c` → `global_functions[2]`（b のスコープで可視）
- 各レベルの `identifier_map` で正しいグローバルインデックスを参照 ✓

#### 3. 子スコープの関数へのアクセス（エラーケース）

```nospace
func: outer() {
  func: inner() {
    func: innermost() { __trace(1); }
  }
  innermost();  # エラー: outer の identifier_map に innermost はない
}
```

- `innermost` は inner スコープの `identifier_map` にのみ存在
- outer からの `resolve_function("innermost")` は、outer と root のスコープしか探索しないため `None` → エラー ✓

#### 4. ネスト関数のホイスティング

```nospace
func: outer() {
  inner();  # 定義より前に呼び出し
  func: inner() { __trace(1); }
}
```

- パス1aで `inner` が先に `global_functions` と `identifier_map` に登録される
- パス2の式変換時に `resolve_function("inner")` で発見可能 ✓

#### 5. ネスト関数内の static 変数

```nospace
func: test() {
  static: counter;
  counter = 0;
  func: inc() {
    counter = counter + 1;
  }
  inc(); inc();
  __assert(counter == 2);
}
```

- `inc` は `global_functions` に登録される
- `initialize_function_statics` がルートスコープの全関数（`inc` 含む）を走査するため、static 変数が正しく初期化される ✓
- `counter` は `test` の `static` 変数。`inc` 内からの `resolve_variable("counter")` は、スコープスタックを辿って `test` のスコープで発見される。この仕組みは変数解決の既存ロジックであり、関数のフラット化とは独立 ✓

### 具体的な実装変更

#### 1. `analyze_internal_with_parent` の引数追加

```rust
fn analyze_internal_with_parent(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    initial_vars: Vec<String>,
    parent_resolver: Option<&ScopeResolver>,
    global_functions: &mut Vec<Function>,        // 追加
    global_function_names: &mut Vec<String>,      // 追加
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>>
```

ルートの `analyze()` が `Vec<Function>` と `Vec<String>` を作成し、再帰呼び出しで共有する。

#### 2. パス1aの変更

```rust
// 変更前: scope.functions に登録
let func_idx = scope.functions.len();
scope.function_names.push(name.clone());
scope.functions.push(placeholder);
scope.identifier_map.insert(name, Function(IdentifierInfo { idx: func_idx }));

// 変更後: global_functions に登録
let global_idx = global_functions.len();
global_functions.push(placeholder);
global_function_names.push(name.clone());
scope.identifier_map.insert(name, Function(IdentifierInfo { idx: global_idx }));
```

`ScopeBuilder.functions` と `ScopeBuilder.function_names` は**使わなくなる**。
`identifier_map` に格納するインデックスがグローバルインデックスになる。

#### 3. パス2（関数本体解析）の変更

```rust
// 変更前
let (s, es) = analyze_internal_with_parent(block, ScopeType::Function, args, Some(&resolver));
scope.functions[func_idx] = Function { ... };

// 変更後
let (s, es) = analyze_internal_with_parent(
    block, ScopeType::Function, args, Some(&resolver),
    global_functions, global_function_names,  // 共有リストを渡す
);
global_functions[global_idx] = Function { ... };
```

#### 4. `ScopeBuilder` の変更

```rust
pub(super) struct ScopeBuilder {
    pub identifier_map: BTreeMap<String, Identifier>,
    pub variables: Vec<Variable>,
    // functions と function_names を削除
    pub static_init_statements: Vec<ExecStatement>,
}
```

`ScopeBuilder::build()` は `functions` と `function_names` を外部から受け取る:

```rust
pub fn build(
    self,
    is_function_scope: bool,
    root_statements: Vec<ExecStatement>,
    functions: Vec<Function>,         // ルートスコープのみ有効
    function_names: Vec<String>,      // ルートスコープのみ有効
) -> Scope
```

ルートスコープの `build` のみ `global_functions` を渡し、非ルートスコープでは空の Vec を渡す。

#### 5. `ScopeResolver::resolve_function` の変更

```rust
pub fn resolve_function(&self, name: &str) -> Option<IdentifierRef> {
    for (_depth, scope_info) in self.scope_stack.iter().rev().enumerate() {
        if let Some(Identifier::Function(info)) = scope_info.func_map.get(name) {
            return Some(IdentifierRef {
                scope_depth: 0,        // 不要だが互換性のため
                local_index: info.idx, // グローバルインデックス
                is_global: true,       // 常にグローバル
            });
        }
    }
    None
}
```

`scope_depth` と `is_global` の計算ロジックが不要になり、単純化される。

#### 6. インタプリタの変更（最小限）

`interpret_call_user_function_by_ref` は**現在のコードがほぼそのまま使える**:

```rust
// 変更前（実質同じコードが2分岐）
let func = if func_ref.is_global {
    &self.root_scope.functions[func_ref.local_index]
} else {
    &self.root_scope.functions[func_ref.local_index]
};

// 変更後（分岐不要）
let func = &self.root_scope.functions[func_ref.local_index];
```

`initialize_function_statics` もルートスコープの `functions` を走査するだけで
ネスト関数を含む全関数の static 変数を初期化できる。

#### 7. compiler_ws の変更

`generate_scope` が `scope.get_function("main")` でメイン関数を取得しているが、
フラット化後も `identifier_map` と `functions` が同じルートスコープにあるため変更不要。

### ボローチェッカーとの関係

`global_functions: &mut Vec<Function>` を `analyze_internal_with_parent` に渡す場合、
同時に `resolver` が `temporary_scope` を借用している。

- `resolver` は `&temporary_scope.identifier_map`、`&temporary_scope.variables` 等を借用
- `global_functions` は別のオブジェクトへの `&mut` 参照

これらは**異なるオブジェクト**を指すため、Rust のボローチェッカーに違反しない。
`temporary_scope` は `scope.identifier_map.clone()` で作成されたスナップショットであり、
`global_functions` とはメモリ上で独立している。

### `Function.scope_depth` フィールドについて

現在 `scope_depth` は未使用（コンパイラ警告あり）。フラット化後は定義位置の情報としても不要になるため、
このフィールドは**削除可能**。

### 変更影響サマリ

| ファイル | 変更内容 | 規模 |
|---|---|---|
| `src/semantic_analyzer/mod.rs` | パス1a/2で `global_functions` を使用、`analyze()` で Vec 作成 | 中 |
| `src/semantic_analyzer/scope.rs` | `ScopeBuilder` から `functions`/`function_names` 削除、`build()` 引数追加、`resolve_function` 単純化 | 中 |
| `src/interpreter/exec.rs` | `interpret_call_user_function_by_ref` の分岐削除 | 小 |
| `src/interpreter/mod.rs` | 変更なし（`initialize_function_statics` は既にルートスコープ走査） | なし |
| `src/compiler_ws/statement.rs` | 変更なし（`get_function("main")` はルートスコープで使用済み） | なし |

## 次のステップ

1. **方針Bを実装**
   - `ScopeBuilder` から `functions`/`function_names` を削除
   - `analyze_internal_with_parent` に `global_functions`/`global_function_names` 引数追加
   - パス1aでグローバルインデックスを使用
   - パス2で `global_functions[global_idx]` に書き込み
   - `analyze()` で `global_functions` を `Scope` に設定
   - `resolve_function` で常に `is_global: true` を返す
   - `interpret_call_user_function_by_ref` の分岐を削除

2. **テスト実行**
   - `cargo test` で全テストが通ることを確認
   - `tmp/test-nested-actual.ns` でスタックオーバーフローが解消されることを確認

## 関連ファイル

- [src/interpreter/exec.rs](../../../src/interpreter/exec.rs) - **主な修正対象**: `interpret_call_user_function_by_ref`, `LocalEnvironment`
- [src/interpreter/mod.rs](../../../src/interpreter/mod.rs) - `interpret_func`, `initialize_function_statics`
- [src/semantic_analyzer/mod.rs](../../../src/semantic_analyzer/mod.rs) - 関数宣言のホイスティング処理（意味解析は正しく動作している）
- [src/semantic_analyzer/scope.rs](../../../src/semantic_analyzer/scope.rs) - Scope と ScopeResolver の定義
- [src/semantic_analyzer/types.rs](../../../src/semantic_analyzer/types.rs) - `IdentifierRef`, `ExecExpression`, `Block` の定義
- [src/interpreter/exec.rs](../../../src/interpreter/exec.rs) - ユーザー定義関数の呼び出し処理

## 備考

- 簡単なテスト（ネスト関数なし）は成功しているため、基本的な関数呼び出しの仕組みは正しく動作している
- スタックオーバーフローはネスト関数を含む場合のみ発生するため、問題はネスト関数の解析または呼び出し処理にある
