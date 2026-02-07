# compile_test.rs リファクタリング設計

## 背景

`tests/compile_test.rs` は nospace ソースコードが Rust のテスト関数内にハードコードされており、保守性に問題がある。一方 `tests/code_test.rs` は `resources/tests/` 以下の外部ファイルと `test-manifest.yaml` による宣言的なテスト定義を採用しており、`build.rs` でテストコードを自動生成している。`compile_test.rs` もこの仕組みに統合すべきである。

## 現状分析

### compile_test.rs のテスト一覧

| テスト関数名 | カテゴリ | 概要 |
|---|---|---|
| `test_compile_empty_main` | compilation-only | 空のmain関数がコンパイルできること、WS文字のみであること |
| `test_compile_return_42` | compilation-only | return文がコンパイルできること |
| `test_compile_debug_string` | debug-output | デバッグ文字列に特定ニーモニックが含まれること |
| `test_compile_arithmetic` | compilation-only | 算術式がコンパイルできること |
| `test_compile_comparison` | compilation-only | 比較演算がコンパイルできること |
| `test_compile_logical` | compilation-only | 論理演算がコンパイルできること |
| `test_compile_variable` | compilation-only | 変数がコンパイルできること |
| `test_compile_no_main` | compile-error | main関数なしでコンパイルエラーになること |
| `test_compile_and_run_puti` | wsc-run | `__puti(42)` → stdout "42" |
| `test_compile_and_run_putc` | wsc-run | `__putc(65)` → stdout "A" |
| `test_compile_and_run_arithmetic` | wsc-run | `__puti(1+2*3)` → stdout "7" |
| `test_compile_and_run_variable` | wsc-run | 変数代入後出力 → stdout "123" |
| `test_compile_and_run_geti` | wsc-run | stdin "10" → stdout "20" |

### code_test.rs の既存カバレッジ

`test-manifest.yaml` には既に `targets: [interpreter, whitespace]` を持つテストが存在し、`build.rs` が `_ws` 接尾辞のテスト関数を自動生成している。これにより以下が既にカバーされている：

- `test_ok_coding_c000_ws` — 基本的なコンパイル＆実行
- `test_literals_num_001_ws` — リテラルのコンパイル
- `test_operators_arith_001_ws` — 算術演算のコンパイル＆実行
- `test_variables_var_basic_001_ws` — 変数のコンパイル＆実行
- `test_io_puti_basic_001_ws` — puti の I/O テスト
- `test_io_putc_basic_001_ws` — putc の I/O テスト
- `test_io_geti_basic_001_ws` — geti の I/O テスト
- `test_io_getc_basic_001_ws` — getc の I/O テスト
- `test_io_combined_001_ws` — 複合 I/O テスト

## 設計方針

### 方針1: wsc-run テストは削除（既存のマニフェストテストで十分）

`compile_test.rs` の wsc-run カテゴリのテスト5件は、`test-manifest.yaml` の既存テスト（`targets: [interpreter, whitespace]`）と完全に重複している：

| compile_test.rs | test-manifest.yaml の同等テスト |
|---|---|
| `test_compile_and_run_puti` | `test_io_puti_basic_001_ws` |
| `test_compile_and_run_putc` | `test_io_putc_basic_001_ws` |
| `test_compile_and_run_arithmetic` | `test_operators_arith_001_ws` |
| `test_compile_and_run_variable` | `test_variables_var_basic_001_ws` |
| `test_compile_and_run_geti` | `test_io_geti_basic_001_ws` |

→ これらは **削除** する。

### 方針2: compilation-only テストを新しいテストタイプとしてマニフェストに統合

compilation-only テスト（コンパイルが成功することだけを検証）は、既存の `targets: [whitespace]` による `_ws` テストが実質的にカバーしている（コンパイル＋実行の成功を検証しており、コンパイル成功も暗黙的にテストされている）。

ただし、以下の固有の検証は `_ws` テストではカバーされない：
- **出力が空白文字のみ** であることの検証（`test_compile_empty_main`）
- **コンパイルは成功するが wsc 実行は不要** なケース

対応案：
- `test_compile_empty_main` の「WS文字のみ検証」は、`test_whitespace_base` 関数内にアサーションを追加すれば、全 whitespace テストで自動的にカバーされる。
- 残りの compilation-only テストは、対応するテストケースに `targets: [interpreter, whitespace]` を追加するだけで十分。

### 方針3: compile-error テストをマニフェストに新タイプとして追加

`test_compile_no_main` のようなコンパイルエラーのテストは、現在の `test-manifest.yaml` にはテストタイプが存在しない。

**新しいテストタイプ `compile_error` を追加する：**

```yaml
- name: test_compile_error_no_main_001
  type: compile_error
  path: compile/no_main_001
  comment: "main 関数がない場合のコンパイルエラー"
```

check.json の形式：
```json
{
  "type": "compile_error",
  "contains": ["main"]
}
```

影響範囲：
- `resources/tests/fails/compile/` ディレクトリの新設
- `build.rs` に `compile_error` タイプのコード生成を追加
- `code_test.rs` に `test_compile_error_base()` 関数を追加
- `TestConfig` enum に `CompileError` バリアントを追加

### 方針4: debug-output テストは Rust 内に残す

`test_compile_debug_string` はデバッグ出力のニーモニック内容を検証するもので、外部ファイル化のメリットが薄い。**compile_test.rs 内に残す**（ただし最小限の1件のみ）。

## 変更計画

### Phase 1: WS文字検証の共通化
- `code_test.rs` の `test_whitespace_base` / `test_whitespace_io_base` に「出力が空白文字のみ」のアサーションを追加

### Phase 2: compile_error テストタイプの追加
1. `code_test.rs` の `TestConfig` に `CompileError` バリアントを追加
2. `code_test.rs` に `test_compile_error_base()` 関数を追加
3. `build.rs` に `compile_error` タイプのコード生成ロジックを追加
4. `resources/tests/fails/compile/` ディレクトリ・テストファイルの作成
5. `test-manifest.yaml` にエントリを追加

### Phase 3: compile_test.rs のテスト削除・縮小
1. wsc-run テスト 5件を削除
2. compilation-only テスト 6件を削除（マニフェストの既存テストの `targets` に `whitespace` を追加して代替）
3. compile-error テスト 1件を削除（Phase 2 のマニフェストエントリで代替）
4. debug-output テスト 1件のみ残す

### Phase 4: compile_test.rs の完全削除の検討
- debug-output テスト 1件のみになった場合、`code_test.rs` 内に直接移動し、`compile_test.rs` を完全に削除する

## 最終的なファイル構成

```
resources/tests/
├── test-manifest.yaml       # compile_error エントリを追加
├── passes/                  # （変更なし）
└── fails/
    ├── syntax/              # （既存）
    ├── semantic/            # （既存）
    ├── runtime/             # （既存）
    └── compile/             # 【新規】コンパイルエラーのテスト
        ├── no_main_001.ns
        └── no_main_001.check.json
```

変更対象ファイル:
- `build.rs` — `compile_error` テストタイプの生成ロジック追加
- `tests/code_test.rs` — `CompileError` / `test_compile_error_base()` 追加、WS文字検証追加
- `resources/tests/test-manifest.yaml` — `compile_error` エントリ追加、一部テストに `targets: whitespace` 追加
- `tests/compile_test.rs` — 大幅縮小（debug テスト 1件のみ、または完全削除）
