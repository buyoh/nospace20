# エントリーポイント名を `main` → `__main` に変更

## 概要

nospace 言語のエントリーポイント関数名を `main` から `__main` に変更する。
`__` で始まる識別子は組み込み識別子として予約されているため、エントリーポイントもこの命名規約に統一する。

## 背景・動機

- 現在 `main` はグローバルスコープで予約されているが、`__` プレフィックスの組み込み識別子とは別扱いになっている
- `__main` にすることで、組み込み識別子の命名規約に統一される
- ユーザー定義関数と組み込み予約語の区別が明確になる

## 変更対象

### Step 1: 言語仕様の更新

対象ファイル:

- `docs/spec.md`
  - 予約語セクション: `main` → `__main` に記述変更
  - 関数定義セクション: `main` 関数の例示と説明文を `__main` に変更
  - コード例中のすべての `func: main()` を `func: __main()` に変更
- `docs/tutorial.md`
  - すべてのコード例とテキスト中の `main` → `__main`
- `docs/optimize.md`
  - dead-code 最適化の説明で `main` → `__main`

### Step 2: コアロジックの変更（Rust ソースコード）

#### 2a: セマンティックアナライザ

- `src/semantic_analyzer/scope.rs` (L386)
  - `name == "main"` → `name == "__main"` に変更

#### 2b: CLI エントリーポイント

- `src/bin/nospace20.rs` (L186)
  - `a.has_function("main")` → `a.has_function("__main")` に変更
  - エラーメッセージ `"error: function 'main' not found"` → `"error: function '__main' not found"` に変更

#### 2c: インタプリタ

- `src/interpreter/mod.rs` (L152-153)
  - `scope.main_function_index` 使用箇所のエラーメッセージ中の `'main'` → `'__main'`

#### 2d: WS コンパイラ

- `src/compiler_ws/mod.rs` (L53)
  - エラーメッセージ `"main function not found"` → `"__main function not found"` に変更
- `src/compiler_ws/builtin.rs`
  - コメント中の `main` → `__main` に変更

#### 2e: オプティマイザ

- `src/optimizer/dead_code.rs`
  - コメント中の `main` → `__main` に変更（ロジックは `main_function_index` を使用しており文字列 `"main"` を直接参照していないため、コメントのみ）

### Step 3: Rust テストコードの変更

#### 3a: セマンティックアナライザテスト

- `src/semantic_analyzer/tests.rs`
  - `"main".to_string()` を `"__main".to_string()` に変更（約20箇所）
  - `scope.get_function("main")` → `scope.get_function("__main")`（約4箇所）

#### 3b: オプティマイザテスト

- `src/optimizer/tests.rs`
  - `interpret_func_testing(&scope, "main")` → `interpret_func_testing(&scope, "__main")`（約30箇所）
  - テスト用 nospace ソースコード中の `func: main()` → `func: __main()` も変更が必要

#### 3c: インタプリタテスト

- `src/interpreter/exec.rs`
  - テスト用コードリテラル中の `func: main()` → `func: __main()`（約2箇所）
  - `scope.get_function("main")` → `scope.get_function("__main")`（約2箇所）

#### 3d: 統合テスト

- `tests/code_test/interpreter_base.rs`
  - `interpret_func_testing(&a, "main")` → `interpret_func_testing(&a, "__main")`（約5箇所）
  - `interpret_func_with_io(&a, "main", ...)` → `interpret_func_with_io(&a, "__main", ...)`（約2箇所）
- `tests/code_test/error_base.rs`
  - `interpret_func_with_io(&a, "main", "")` → `interpret_func_with_io(&a, "__main", "")`
- `tests/ignore_debug_test.rs`
  - `interpret_func_with_config(&scope, "main", config)` → `interpret_func_with_config(&scope, "__main", config)`（約6箇所）
- `tests/compile_test.rs`
  - `errors[0].message.contains("main")` → `errors[0].message.contains("__main")`

### Step 4: リソーステストファイルの変更

- `resources/tests/` 以下の `.ns` ファイル（約223ファイル）
  - `func: main()` / `func:main()` → `func: __main()` / `func:__main()` に一括置換
  - テスト用 nospace ソースコード内の `main` 文字列を `__main` に変更
- `resources/tests/README.md`
  - コード例中の `func: main()` → `func: __main()` に変更

### Step 5: AI ドキュメント・その他ドキュメントの更新

- `docs-ai/` 内で `main` をエントリーポイントとして言及している箇所の更新
- `resources/tests/README.md` の更新

## 影響しないファイル

- `resources/tests_ws/` の `.wsa` ファイル: コメント内の `main` はラベル名の説明であり、nospace エントリーポイントとは無関係
- `src/semantic_analyzer/scope.rs` の `main_function_index` フィールド名: Rust ソースコード内の変数名・フィールド名はそのまま保持可能（nospace 言語の識別子ではないため）
- `src/compiler_ws/builtin.rs` のロジック: `main_function_index` を使っているため、文字列 `"main"` のハードコーディングはない
- `src/optimizer/dead_code.rs` のロジック: 同上、`main_function_index` を参照

## 注意事項

- `main_function_index` という Rust のフィールド名/変数名は変更しない。これは Rust コード内部の名前であり、nospace 言語の識別子名とは無関係。変更すると不必要に大きな差分になる。
- テストファイルの一括置換は `sed` コマンドでの機械的置換が適切。`func:main` と `func: main` の両パターンに対応する必要がある。
- `"main"` 文字列の置換では、nospace のエントリーポイントを指す箇所のみを変更し、Rust の一般的な `main` 関数への言及と混同しないよう注意。

## 確認方法

- `cargo test` で全テストが通ること
- `cargo build` でビルドが通ること
- リソーステストの全 `.ns` ファイルに `func: main` が残っていないことを grep で確認
