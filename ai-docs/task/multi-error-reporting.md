# 意味解析における複数エラー報告

## 概要

現在、意味解析（`semantic_analyzer`）でコンパイルエラーが発生した場合、最初の1つのエラーのみが報告され処理が中断される。
字句解析・構文解析は既に複数エラー収集に対応しているが、意味解析だけが未対応である。
本タスクでは、意味解析で複数のエラーを収集し、一度に報告できるようにする。

## 現状分析

### 各フェーズのエラー収集能力

| フェーズ | Err 型 | 複数エラー収集 | 最初のエラーで中断 |
|---|---|---|---|
| 字句解析 (token_parser) | `Vec<CodeParseError>` | **対応済** | No |
| 構文解析 (tree_parser) | `Vec<CodeParseError>` | **対応済** | No |
| 意味解析 (semantic_analyzer) | `Vec<CodeParseError>` | **未対応**（形式のみ Vec） | Yes |
| CLI 表示 | — | `.take(3)` 最大3件 | — |
| WASM 表示 | — | 全件表示 | — |

### 意味解析の問題箇所

意味解析は型シグネチャ上 `Result<..., Vec<CodeParseError>>` を返すが、実質的に常に `vec![single_error]` しか入らない。

#### 1. `convert_to_exec_expression_with_resolver` 関数（mod.rs L33〜）

`?` 演算子による即座の return が 10 箇所以上存在する。

```rust
// 例: 未定義変数
let var_ref = parent_resolver
    .resolve_variable(v)
    .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", v))])?;  // ← ここで中断
```

エラーが発生しうるケース:
- 未定義変数の参照（`undefined variable`）
- 未定義関数の呼び出し（`undefined function`）
- 非配列変数への配列アクセス（`is not an array`）
- `&` 演算子の不正な使用（`reference operator can only be applied to ...`）
- 再帰的な子式の処理中のエラー

#### 2. `analyze_internal_with_parent` 関数（mod.rs L242〜）

パス1a（関数宣言スキャン）:
```rust
scope.add_identifier(name, Identifier::Function(...))?;  // 重複名で中断
```

パス1b（変数宣言収集）:
```rust
scope.add_variable(name, Variable { ... })?;  // 重複名で中断
```

パス2（文の変換）:
```rust
convert_to_exec_expression_with_resolver(init, &resolver)?;  // 変換エラーで中断
analyze_internal_with_parent(block, ...)?;  // 関数本体エラーで中断
```

エラーが発生しうるケース:
- 識別子の重複定義（`the name 'x' is already used`）
- `return`/`continue`/`break` の不正な位置（`outside of function`）
- 子式・子文の処理中のエラー（上記1由来）

## 設計方針

### アプローチ: エラーコレクタパターン + ダミー値

エラー発生時に即座に中断せず、以下の方針で処理を継続する:

1. **エラーコレクタ**: `&mut Vec<CodeParseError>` をエラー収集先として関数間で受け渡す
2. **ダミー値**: エラー発生時にダミーの `ExecExpression`/`ExecStatement` を生成して解析を継続
3. **最終チェック**: 解析完了後、エラーが1つでもあれば `Err(collected_errors)` を返す

このアプローチの利点:
- 字句解析・構文解析が `Expression::Invalid` を使って回復するパターンと一貫性がある
- 戻り値の型を大きく変更せずに実現できる
- 1つのエラーが後続の解析に影響を与えにくい

### `ExecExpression::Invalid` の導入

エラー回復用のダミーバリアントを追加する。

```rust
// src/semantic_analyzer/types.rs
pub enum ExecExpression {
    // ... 既存のバリアント ...
    /// エラー回復用（意味解析でエラーが検出された場合のプレースホルダー）
    Invalid,
}
```

`ExecExpression::Invalid` がインタプリタやコンパイラで実行されることはない（エラーがあれば実行に進まないため）。
ただし念のため、インタプリタ側にも `Invalid` のマッチアームを追加し panic させる。

## 詳細設計

### モジュール別変更一覧

| モジュール | ファイル | 変更内容 |
|---|---|---|
| semantic_analyzer/types | types.rs | `ExecExpression::Invalid` バリアント追加 |
| semantic_analyzer/mod | mod.rs | エラーコレクタパターンへのリファクタリング |
| semantic_analyzer/scope | scope.rs | `add_identifier`/`add_variable` の戻り値変更 |
| interpreter | mod.rs | `ExecExpression::Invalid` の panic マッチアーム追加 |
| compiler_ws | 関連ファイル | `ExecExpression::Invalid` の panic マッチアーム追加 |

### ステップ1: `ExecExpression::Invalid` の追加

[src/semantic_analyzer/types.rs](../../src/semantic_analyzer/types.rs) に `Invalid` バリアントを追加。

```rust
pub enum ExecExpression {
    // ... 既存 ...
    /// 意味解析エラー時のプレースホルダー
    Invalid,
}
```

### ステップ2: `convert_to_exec_expression_with_resolver` のリファクタリング

**Before（現在）:**
```rust
fn convert_to_exec_expression_with_resolver(
    expr: &Box<Expression>,
    parent_resolver: &ScopeResolver,
) -> Result<Box<ExecExpression>, Vec<CodeParseError>> {
    // ... ?演算子で即return ...
}
```

**After（変更後）:**
```rust
fn convert_to_exec_expression_with_resolver(
    expr: &Box<Expression>,
    parent_resolver: &ScopeResolver,
    errors: &mut Vec<CodeParseError>,
) -> Box<ExecExpression> {
    // エラー時は errors.push(...) して ExecExpression::Invalid を返す
}
```

変更パターン:

```rust
// Before
let var_ref = parent_resolver
    .resolve_variable(v)
    .ok_or_else(|| vec![code_parse_error!(...)])?;
Ok(Box::new(ExecExpression::Variable(var_ref)))

// After
match parent_resolver.resolve_variable(v) {
    Some(var_ref) => Box::new(ExecExpression::Variable(var_ref)),
    None => {
        errors.push(code_parse_error!(...));
        Box::new(ExecExpression::Invalid)
    }
}
```

再帰呼び出しについて:
```rust
// Before
let exec_r = convert_to_exec_expression_with_resolver(&r, parent_resolver)?;

// After
let exec_r = convert_to_exec_expression_with_resolver(&r, parent_resolver, errors);
// エラーがあっても Invalid が返るだけなので、処理は継続できる
```

### ステップ3: `analyze_internal_with_parent` のリファクタリング

**Before:**
```rust
fn analyze_internal_with_parent(...) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    // パス1a
    scope.add_identifier(...)?;  // 中断
    // パス1b
    scope.add_variable(...)?;  // 中断
    // パス2
    convert_to_exec_expression_with_resolver(...)?;  // 中断
}
```

**After:**
```rust
fn analyze_internal_with_parent(...) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    let mut errors = Vec::new();

    // パス1a: 重複は errors に追加し、スキップ
    if let Err(mut errs) = scope.add_identifier(...) {
        errors.append(&mut errs);
        // 重複した関数宣言はスキップして続行
    }

    // パス1b: 重複は errors に追加し、スキップ
    if let Err(mut errs) = scope.add_variable(...) {
        errors.append(&mut errs);
        // 重複した変数宣言はスキップして続行
    }

    // パス2: 式変換はエラーコレクタを使用
    let exec = convert_to_exec_expression_with_resolver(init, &resolver, &mut errors);
    exec_statements.push(ExecStatement::Expression(exec));

    // 関数本体の解析もエラー継続
    match analyze_internal_with_parent(block, ...) {
        Ok((s, es)) => { /* 正常処理 */ }
        Err(mut errs) => {
            errors.append(&mut errs);
            // ダミーの関数本体を設定
        }
    }

    // 最終: エラーがあれば Err
    if errors.is_empty() {
        Ok((scope, exec_statements))
    } else {
        Err(errors)
    }
}
```

#### パス1a/1b の重複処理の詳細

重複が検出された場合、その宣言をスキップする。後続の参照は最初に登録された定義を使う。

```rust
// パス1a: 関数重複
Statement::FunctionDeclaration(name, _, _) => {
    let global_idx = global_functions.len();
    global_function_names.push(name.clone());
    global_functions.push(/* placeholder */);
    if let Err(mut errs) = scope.add_identifier(name, Identifier::Function(FunctionIndex(global_idx))) {
        errors.append(&mut errs);
        // 重複したのでプレースホルダーは追加されたがidentifier_mapには未登録
        // → 後のパス2で最初の定義が使われる
    }
}
```

#### パス2: 文の変換での注意点

- `return`/`continue`/`break` の不正位置はエラー収集して続行可能（文自体はスキップ）
- 関数本体の解析が失敗した場合、ダミーの `Block` を生成して `global_functions` を更新

### ステップ4: インタプリタ/コンパイラの `Invalid` 対応

```rust
// src/interpreter/mod.rs
ExecExpression::Invalid => {
    unreachable!("ExecExpression::Invalid should not reach interpreter")
}
```

```rust
// src/compiler_ws/ の該当ファイル
ExecExpression::Invalid => {
    unreachable!("ExecExpression::Invalid should not reach compiler")
}
```

### ステップ5: CLI 表示の調整（任意）

現在 `.take(3)` で最大3件表示。意味解析で複数エラーが返るようになった場合も、この制限はそのまま適用される。
必要に応じて表示件数を増やすか、設定可能にすることも検討可能だが、本タスクのスコープ外とする。

## カスケーディングエラーへの考慮

1つ目のエラーが原因で発生する2次的なエラー（カスケーディングエラー）を軽減する設計:

- **未定義変数/関数**: `Invalid` ノードが生成されるが、親の式も `Invalid` に伝播するわけではなく、各部分式は独立してエラーチェックされる。同じ未定義名を複数箇所で使っている場合は、使用箇所ごとに報告される（これは有用な情報）。
- **重複定義**: 最初の定義が有効なまま解析が続くため、後続の参照は正常に解決される。重複定義自体のエラーのみ報告される。
- **不正な文（return等）**: 独立したエラーなのでカスケーディングの心配なし。

## テスト計画

### Unit テスト（semantic_analyzer/tests.rs に追加）

1. **複数の未定義変数**: 2つ以上の未定義変数を含むコードで、全てのエラーが報告されることを確認
2. **重複定義 + 未定義参照**: 識別子重複と未定義参照が同時に存在するケースで両方報告されることを確認
3. **関数本体内のエラー + 別の関数のエラー**: 異なる関数内のエラーが独立して報告されることを確認
4. **既存テストの互換性**: 1つのエラーのみの場合でも従来通り正しく報告されることを確認

### Large テスト（resources/tests/ に追加）

複数エラーの検出を確認するテストケースを追加。
（テストフレームワークが `Err` ケースの検証に対応しているか要確認）

## 作業見積もり

| ステップ | 内容 | 規模 |
|---|---|---|
| 1 | `ExecExpression::Invalid` 追加 | 小 |
| 2 | `convert_to_exec_expression_with_resolver` リファクタリング | 中〜大 |
| 3 | `analyze_internal_with_parent` リファクタリング | 中 |
| 4 | インタプリタ/コンパイラの対応 | 小 |
| 5 | テスト作成 | 中 |

全体規模: **中規模**（主に semantic_analyzer/mod.rs への変更が中心）
