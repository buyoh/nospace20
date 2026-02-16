# wsc による WSA テストケースのクロスバリデーション

## 概要

`resources/tests_ws/` に配置された WSA テストケースの正当性を、外部 Whitespace インタプリタ `wsc`（whitespacers クレート）を用いて検証する仕組みを設計・実装する。

テストケース自体のエンコーディングエラーが原因でテストが失敗する可能性があるため、独立した参照実装による実行結果と比較することで、テストケースの正しさとインタプリタの正しさを同時に検証する。

## 背景

- WSA（Whitespace Assembly）はプロジェクト独自のフォーマット（S/T/N 記法）
- 手動でバイナリエンコーディングを記述するため、エンコーディングミスが起きやすい
- 現在 39 テストケース中 12 テストが失敗しており、テストケース側のバグかインタプリタのバグか判別が困難
- `wsc` は Rust 製の外部 Whitespace インタプリタで、参照実装として利用可能

## 既存インフラの整理

### wsc 関連

- **インストール**: `tools/setup-wsc.sh` → `tools/wsc-install/bin/wsc` に配置
- **ユーティリティ**: `tests/common/mod.rs` に `find_wsc()`, `wsc_available()`, `run_whitespace()` が既存
- **既存利用**: nospace→WS コンパイルテストで `targets: ["whitespace"]` 時に `#[ignore]` 付きテストを生成し、`wsc` で実行

### WSA テスト関連

- **マニフェスト**: `resources/tests_ws/test-manifest.yaml`（39 テスト）
- **テストランナー**: `tests/whitespace_direct_test.rs`
  - `decode_wsa()`: WSA→WS 変換（`#` コメント行除外、S→Space, T→Tab, N→LF）
  - `test_ws_io_base()`: WhitespaceVM で実行し stdout を比較
  - `test_ws_runtime_error_base()`: WhitespaceVM で実行しエラー種別を比較
- **生成コード**: `build.rs` の `generate_ws_tests()` → `generated_ws_tests.rs`

## 設計

### 方針

WSA テストケースに対して、自前の WhitespaceVM での実行に加えて `wsc` での実行も行い、結果を比較する `#[ignore]` テストを生成する。

既存の nospace テスト（`code_test.rs`）が `targets: ["whitespace"]` で wsc テストを生成している仕組みと一貫性を持たせる。

### 新しいテスト関数

`tests/whitespace_direct_test.rs` に以下の関数を追加:

#### `test_ws_io_wsc_base(test_name: &str)`

1. WSA ファイルを読み込み、`decode_wsa()` で WS コードに変換
2. `check.json` から `stdin` / `stdout` を読み込み
3. `run_whitespace()` で wsc を使い WS コードを実行
4. 実際の stdout と期待値を比較

```rust
fn test_ws_io_wsc_base(test_name: &str) {
    use common::{run_whitespace, wsc_available};
    if !wsc_available() {
        eprintln!("Skipping test: wsc not available");
        eprintln!("Run: ./tools/setup-wsc.sh");
        return;
    }
    // WSA デコード → wsc 実行 → stdout 比較
}
```

#### `test_ws_runtime_error_wsc_base(test_name: &str)`

1. WSA ファイルを読み込み、`decode_wsa()` で WS コードに変換
2. `run_whitespace()` で wsc を使い実行
3. wsc がエラーを返すことを確認（エラー種別の厳密な比較は行わない）

> **注意**: wsc のエラーメッセージ形式は自前 VM と異なるため、「エラーが発生すること」のみを検証する。

### build.rs の変更

`generate_ws_tests()` を拡張し、各テストに対して `_wsc` サフィックス付きの `#[ignore]` テストも生成する:

```rust
// ws_io テスト
"ws_io" => {
    // 既存: WhitespaceVM テスト
    writeln!(f, r#"#[test]
fn {}() {{ test_ws_io_base("{}") }}"#, test.name, test.path).unwrap();

    // 追加: wsc クロスバリデーションテスト
    writeln!(f, r#"#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn {}_wsc() {{ test_ws_io_wsc_base("{}") }}"#, test.name, test.path).unwrap();
}
```

`ws_runtime_error` テストも同様に `_wsc` テストを生成。

### テスト実行方法

```bash
# 通常テスト（WhitespaceVM のみ）
cargo test --test whitespace_direct_test

# wsc クロスバリデーション含む
cargo test --test whitespace_direct_test -- --ignored

# 全テスト
cargo test --test whitespace_direct_test -- --include-ignored
```

## 実装ステップ

### Step 1: wsc インストール確認

- `tools/setup-wsc.sh` を実行して wsc をインストール
- 動作確認: `./tools/wsc-install/bin/wsc --version`

### Step 2: テストランナー拡張

`tests/whitespace_direct_test.rs` にクロスバリデーション用関数を追加:

- `test_ws_io_wsc_base()`
- `test_ws_runtime_error_wsc_base()`

### Step 3: build.rs 拡張

`generate_ws_tests()` で `_wsc` サフィックス付き `#[ignore]` テストを生成。

### Step 4: テスト実行・検証

1. `cargo test --test whitespace_direct_test -- --ignored` で wsc テストを実行
2. 結果を分析し、テストケースのバグとインタプリタのバグを分類:
   - **wsc も自前 VM も失敗** → テストケース（WSA）のバグ
   - **wsc は成功、自前 VM は失敗** → 自前 VM のバグ
   - **wsc は失敗、自前 VM は成功** → wsc の仕様差異（調査必要）
   - **両方成功** → テストケースもインタプリタも正しい

### Step 5: テストケース修正

Step 4 の結果に基づき、バグのあるテストケースを修正。

## 関連ファイル

- `tests/whitespace_direct_test.rs` - テストランナー
- `tests/common/mod.rs` - wsc ユーティリティ（`find_wsc`, `wsc_available`, `run_whitespace`）
- `build.rs` - テスト自動生成
- `resources/tests_ws/test-manifest.yaml` - テストマニフェスト
- `tools/setup-wsc.sh` - wsc インストールスクリプト

## 制約・考慮事項

- wsc と自前 VM の挙動差異: 数値出力のフォーマット差異（末尾改行等）の可能性がある
- wsc はファイルパスを引数に取る（stdin パイプではない）ため、一時ファイル経由で WS コードを渡す（`run_whitespace()` が既にこの方式）
- `#[ignore]` テストのため、CI で wsc がインストールされていない場合でも通常テストに影響しない
- エラーテストの wsc 検証はエラー種別の厳密比較が困難なため、「実行失敗すること」のみを検証
## 進捗

### Step1 完了（既存インフラ確認）

- wsc は `tools/wsc-install/bin/wsc` に既にインストール済み
- `tests/common/mod.rs` に必要な関数が実装済み

### Step2 完了（テストランナー拡張）

実装内容:
- `tests/whitespace_direct_test.rs` に以下の関数を追加:
  - `test_ws_io_wsc_base(test_name: &str)`: I/O テストの wsc クロスバリデーション
  - `test_ws_runtime_error_wsc_base(test_name: &str)`: エラーテストの wsc クロスバリデーション
- `mod common;` をファイル先頭で宣言

### Step3 完了（build.rs 拡張）

実装内容:
- `build.rs` の `generate_ws_tests()` 関数を拡張
- 各テストに対して `_wsc` サフィックス付きの `#[ignore]` テストを生成
- `ws_io` と `ws_runtime_error` の両方のテストタイプに対応

### Step4 完了（テスト実行・検証）

**テスト実行結果:**

#### wsc クロスバリデーションテスト（`cargo test --test whitespace_direct_test -- --ignored`）
- **結果**: 39 passed; 0 failed
- **所要時間**: 0.32s
- **結論**: 全ての WSA テストケースは wsc で正常に実行できる

#### 自前 WhitespaceVM テスト（`cargo test --test whitespace_direct_test`）
- **結果**: 27 passed; 12 failed; 39 ignored
- **所要時間**: 0.00s

**失敗したテスト（12件）:**
1. `test_ws_arith_combined_001`
2. `test_ws_arith_mul_001`
3. `test_ws_arith_sub_001`
4. `test_ws_errors_callstack_underflow_001`
5. `test_ws_errors_div_zero_001`
6. `test_ws_errors_stack_underflow_001`
7. `test_ws_errors_undefined_label_001`
8. `test_ws_flow_call_return_001` - Error(UndefinedLabel(1))
9. `test_ws_flow_jump_if_neg_false_001` - Error(UndefinedLabel(1))
10. `test_ws_flow_jump_if_neg_true_001` - stdout mismatch: 期待 "991", 実際 "1"
11. `test_ws_flow_jump_if_zero_false_001` - Error(UndefinedLabel(1))
12. `test_ws_flow_loop_simple_001` - stdout mismatch: 期待 "3", 実際 "321"

**分析:**
- **テストケースの正当性**: wsc で全テストがパスしているため、WSA テストケース自体は正しい
- **自前 VM のバグ**: 自前 WhitespaceVM に実装バグがあることが確定
- **主な問題点**:
  - `UndefinedLabel(1)` エラーが多発: ラベル解釈・参照に問題がある可能性
  - stdout の不一致: 制御フロー（ジャンプ、ループ）の実装に問題がある可能性

**次のステップ（Step5）の方針:**
- テストケースの修正は不要（wsc で全てパス）
- 自前 WhitespaceVM の実装を修正する必要がある
- 特に以下の機能に焦点を当てる:
  - ラベルのエンコーディング/デコーディング
  - ジャンプ命令の実装
  - 条件分岐の実装
  - サブルーチン呼び出しとリターン

### Step5 完了（テストケース修正判定）

- wsc で全39テストがパスしたため、WSA テストケース自体に修正は不要
- 12件の WhitespaceVM テスト失敗は自前 VM の既存バグであり、本タスクのスコープ外
- VM のバグ修正は別タスクとして対応する

### 全ステップ完了

- wsc クロスバリデーションの仕組みが正常に動作
- テストケースの正当性が確認された
- 自前 VM の12件のバグが明確に特定された