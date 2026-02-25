# build.rs の分割リファクタリング

## 概要

`build.rs`（523行）が肥大化しているため、ロジックを `src_build/` ディレクトリ以下のモジュールに分割する。

## 現状分析

### build.rs の構成

| 行範囲 | 内容 | 行数 |
|---------|------|------|
| 1-28 | `use` 宣言、`TestManifest` / `TestCase` 構造体定義 | 28 |
| 30-37 | `main()` エントリポイント | 8 |
| 39-45 | `format_comment_line()` ユーティリティ | 7 |
| 47-97 | `TargetFlags` 構造体・`from_test_case` 実装 | 51 |
| 99-121 | `generate_nospace_tests()` マニフェスト読み込み・振り分け | 23 |
| 123-233 | `write_success_tests()` success テストコード生成 | 111 |
| 235-331 | `write_success_io_tests()` success_io テストコード生成 | 97 |
| 333-349 | `write_error_test()` エラー系テストコード生成 | 17 |
| 351-441 | `generate_ws_tests()` Whitespace テスト生成 | 91 |
| 443-523 | `generate_alloc_tests()` alloc テスト生成 | 81 |

### 論理的なまとまり

1. **共通型・ユーティリティ**: `TestManifest`, `TestCase`, `format_comment_line`, `TargetFlags`
2. **nospace テスト生成**: `generate_nospace_tests`, `write_success_tests`, `write_success_io_tests`, `write_error_test`
3. **Whitespace テスト生成**: `generate_ws_tests`
4. **alloc テスト生成**: `generate_alloc_tests`

## 設計

### ディレクトリ構成

```
build.rs                        # エントリポイント（main + mod 宣言のみ）
src_build/
  common.rs                     # 共通型・ユーティリティ
  gen_nospace_tests.rs           # nospace テスト生成
  gen_ws_tests.rs                # Whitespace テスト生成
  gen_alloc_tests.rs             # alloc テスト生成
```

### モジュール間依存

```
build.rs
  ├── src_build::common          (型定義・ユーティリティ)
  ├── src_build::gen_nospace_tests   (common を使用)
  ├── src_build::gen_ws_tests        (common を使用)
  └── src_build::gen_alloc_tests     (common を使用)
```

### 各ファイルの内容

#### build.rs（エントリポイント）

- `#[path = "src_build/common.rs"] mod common;` 等のモジュール宣言
- `main()` 関数のみ残す
- 各ジェネレータ関数を対応モジュールから呼び出す

Rust の `build.rs` は独立したバイナリとしてコンパイルされるため、`src/` 以下のモジュールシステムとは独立している。`#[path = "..."]` アトリビュートを使ってモジュール参照する。

#### src_build/common.rs

- `use` 宣言（`serde::Deserialize`）
- `TestManifest` 構造体
- `TestCase` 構造体
- `format_comment_line()` 関数
- `TargetFlags` 構造体と `impl`

公開範囲: すべて `pub` とする（build.rs 内の他モジュールから参照されるため）。

#### src_build/gen_nospace_tests.rs

- `use crate::common::*;`
- `generate_nospace_tests()` 関数（`pub`）
- `write_success_tests()` 関数（プライベート）
- `write_success_io_tests()` 関数（プライベート）
- `write_error_test()` 関数（プライベート）

#### src_build/gen_ws_tests.rs

- `use crate::common::*;`
- `generate_ws_tests()` 関数（`pub`）

#### src_build/gen_alloc_tests.rs

- `use crate::common::*;`
- `generate_alloc_tests()` 関数（`pub`）

### 移行手順

1. `src_build/` ディレクトリを作成
2. `src_build/common.rs` を作成（共通型を移動）
3. `src_build/gen_nospace_tests.rs` を作成（nospace テスト生成ロジックを移動）
4. `src_build/gen_ws_tests.rs` を作成（Whitespace テスト生成ロジックを移動）
5. `src_build/gen_alloc_tests.rs` を作成（alloc テスト生成ロジックを移動）
6. `build.rs` をエントリポイントのみに書き換え
7. `cargo build` でビルド確認
8. `cargo test` でテスト確認

### 注意点

- `build.rs` は Cargo のビルドスクリプトであり、`src/` とは別のコンパイル単位である
- `#[path = "src_build/xxx.rs"]` を使ってモジュールパスを指定する（build.rs にとって `src_build/` はデフォルトのモジュール検索パスではないため）
- `Cargo.toml` の変更は不要（`build = "build.rs"` はデフォルト設定）
- 動作の変更は一切なく、純粋なリファクタリングである

## ステータス

- [ ] 設計完了
- [ ] 実装
- [ ] テスト確認
