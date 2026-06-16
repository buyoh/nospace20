# LanguageStd を Standard のみに簡素化し、Alloc 拡張をデフォルト化

## 概要

LanguageStd を Standard のみに整理し、TargetExtension::Alloc をデフォルトで有効化する変更。

## 変更内容

### 1. LanguageStd の整理

- `LanguageStd::Ws` を削除
  - Whitespace へのコンパイルは `--mode=compile --target=ws` で可能（`--std=ws` は不要に）
- `LanguageStd::Min` をコメントアウト
  - 将来の実装予定として予約
- `LanguageStd::Standard` のみを残す

### 2. TargetExtension::Alloc のデフォルト化

- `--no-std-ext` オプションを追加してデフォルト拡張を無効化可能に
- `CliCompileArgs::build_target_extensions()` ヘルパーメソッドを追加
  - デフォルトで `Alloc` を含める
  - `--no-std-ext` が指定された場合は追加しない

### 3. バリデーションの簡素化

- `target=ws/mnemonic` 時に `std=ws` を要求していたチェックを削除
- `std=min` の未対応チェックを削除（enum から削除されたため不要）
- `--std-ext alloc` の制約チェックを削除（すべてのモードで使用可能に）

## 影響を受けたファイル

- `src/compile_property.rs`: LanguageStd の定義とバリデーション
- `src/cli_utils.rs`: CliStd の定義、build_target_extensions() の追加
- `src/bin/nospace20.rs`: build_target_extensions() の使用
- `src/wasm_api/api.rs`: "ws" std のサポート削除
- `examples/ws_profiler.rs`: build_target_extensions() の使用

## テスト結果

- 全ユニットテスト（415 tests）パス
- 全 code_test（1479 tests）パス
- 全 alloc_test（15 tests）パス
- デフォルトで alloc が有効になることを手動確認
  - デフォルト: 364 行の Whitespace コード生成
  - `--no-std-ext`: 97 行の Whitespace コード生成
  - `--std-ext alloc`: 364 行の Whitespace コード生成（デフォルトと同じ）

## 完了日

2026-03-08

## コミット

ef70923 - Simplify LanguageStd to Standard only and make Alloc extension default
