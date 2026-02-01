# YAMLベースのテスト自動生成

## 目的

現在の `tests/code_test.rs` は手動でマクロ呼び出しを追加する必要があり、保守性が低い。
YAMLファイルでテストケースを定義し、ビルド時に自動的にテストコードを生成する仕組みを導入する。

## 利点

- テスト追加が簡単（YAMLに1エントリ追加するだけ）
- コメントによる補足情報を記述可能
- テストファイルのサイズ削減
- 保守性向上

## YAMLフォーマット

```yaml
# resources/tests/test-manifest.yaml
tests:
  # Legacy tests (backward compatibility)
  - name: test_ok_coding_c000
    type: success
    path: c000
    comment: "Basic arithmetic test"
  
  - name: test_ok_coding_c001
    type: success
    path: c001
  
  # DISABLED: hangs (break/continue issue)
  # - name: test_ok_coding_c002
  #   type: success
  #   path: c002
  
  # Literals
  - name: test_literals_num_001
    type: success
    path: literals/num_001
    comment: "Numeric literal parsing"
  
  # I/O tests
  - name: test_legacy_001
    type: success_io
    path: legacy/legacy_001
    comment: "Basic I/O with puti/geti"
  
  # Syntax errors
  - name: test_syntax_error_invalid_token_001
    type: syntax_error
    path: fails/syntax/invalid_token_001
    comment: "Invalid token detection"
```

### フィールド定義

- `name`: テスト関数名（`fn name() { ... }` として生成）
- `type`: テストタイプ（`success`, `success_io`, `syntax_error`）
- `path`: テストファイルのパス（`resources/tests/passes/` または `resources/tests/fails/syntax/` からの相対パス）
- `comment`: （オプション）テストの説明・コメント

## 実装アプローチ

### 1. ビルドスクリプト (`build.rs`)

- YAMLファイルを読み込み
- テストコードを生成（Rustコード文字列）
- `OUT_DIR/generated_tests.rs` に書き出し

### 2. テストファイル (`tests/code_test.rs`)

- ヘルパー関数（`test_ok_coding_base`, `test_ok_coding_io_base`, `test_syntax_error_base`）を維持
- 生成されたテストコードを `include!` マクロでインクルード

### 3. YAML設定ファイル (`resources/tests/test-manifest.yaml`)

- 全テストケースを定義

## ファイル構成

```
nospace20/
├── build.rs                              # 新規作成
├── Cargo.toml                            # 依存関係追加
├── resources/
│   └── tests/
│       └── test-manifest.yaml            # 新規作成
└── tests/
    └── code_test.rs                      # 修正（マクロ呼び出しを削除、includeに変更）
```

## 実装手順

1. `Cargo.toml` に `serde_yaml` を `build-dependencies` として追加
2. `resources/tests/test-manifest.yaml` を作成（既存のテストを移行）
3. `build.rs` を作成（YAML読み込み + コード生成）
4. `tests/code_test.rs` を修正（生成コードをインクルード）
5. テスト実行で動作確認

## 実装状態

- [ ] Cargo.toml 修正
- [ ] test-manifest.yaml 作成
- [ ] build.rs 作成
- [ ] code_test.rs 修正
- [ ] 動作確認

## 注意事項

- YAMLファイルの変更を検出するため、`build.rs` で `rerun-if-changed` を設定
- `disabled_` で始まるテストはYAMLでコメントアウトして除外
- 後方互換性のため、既存のテストケースは全て移行
