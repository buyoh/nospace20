# WhitespaceVM テスト失敗の修正

## 概要

`cargo test --test whitespace_direct_test` で 12 件のテストが失敗する。
原因は **テストデータ（.wsa ファイル）の Whitespace 数値エンコーディングの誤り** と **test-manifest.yaml のパス指定の誤り** の 2 つ。

パーサ（`src/whitespace/parser.rs`）やインタプリタ（`src/whitespace/interpreter.rs`）の実装にはバグはない。

## 失敗テスト一覧

| No. | テスト名 | 原因カテゴリ |
|-----|---------|-------------|
| 1 | test_ws_arith_sub_001 | WSA エンコーディング誤り |
| 2 | test_ws_arith_mul_001 | WSA エンコーディング誤り |
| 3 | test_ws_arith_combined_001 | WSA エンコーディング誤り |
| 4 | test_ws_flow_call_return_001 | WSA エンコーディング誤り |
| 5 | test_ws_flow_jump_if_zero_false_001 | WSA エンコーディング誤り |
| 6 | test_ws_flow_jump_if_neg_false_001 | WSA エンコーディング誤り |
| 7 | test_ws_flow_jump_if_neg_true_001 | WSA エンコーディング誤り |
| 8 | test_ws_flow_loop_simple_001 | WSA エンコーディング誤り |
| 9 | test_ws_errors_stack_underflow_001 | マニフェストパス誤り |
| 10 | test_ws_errors_div_zero_001 | マニフェストパス誤り |
| 11 | test_ws_errors_callstack_underflow_001 | マニフェストパス誤り |
| 12 | test_ws_errors_undefined_label_001 | マニフェストパス誤り |

## 原因 1: WSA ファイルの Whitespace 数値エンコーディング誤り（8 テスト）

### 背景: Whitespace の数値フォーマット

Whitespace の数値は以下の形式でエンコードされる:

```
[符号(S=正, T=負)][2進数字(S=0, T=1, MSB順)][N(終端)]
```

例: 数値 `10` = 2進 `1010` → `S(正) T(1) S(0) T(1) S(0) N(終端)` = `STSTSN`

テストファイルでは、符号ビットの欠落や2進桁の不足により意図しない値がエンコードされている。

### 具体的なバグと修正（全 11 箇所）

#### パターン A: 正符号ビット `S` の欠落

Push や Label 定義で先頭の `S`（正符号）が欠落し、最初のデータビット `T` が負符号として解釈される。

| # | ファイル | コメント | 誤 WSA | 実際の値 | 正しい WSA | 正しい値 |
|---|---------|---------|--------|---------|-----------|---------|
| 1 | `arith/sub_001.wsa` | Push 10 | `SSTSTSN` | -2 | `SSSTSTSN` | +10 |
| 2 | `arith/combined_001.wsa` | Push 2 | `SSTSN` | -0 | `SSSTSN` | +2 |
| 3 | `flow/jump_if_zero_false_001.wsa` | Push 2 | `SSTSN` | -0 | `SSSTSN` | +2 |
| 4 | `flow/jump_if_neg_false_001.wsa` | Push 2 | `SSTSN` | -0 | `SSSTSN` | +2 |
| 5 | `flow/call_return_001.wsa` | Label 1 | `NSSTN` | label 0 | `NSSSTN` | label 1 |
| 6 | `flow/jump_if_zero_false_001.wsa` | Label 1 | `NSSTN` | label 0 | `NSSSTN` | label 1 |
| 7 | `flow/jump_if_neg_false_001.wsa` | Label 1 | `NSSTN` | label 0 | `NSSSTN` | label 1 |
| 8 | `flow/loop_simple_001.wsa` | Label 1 | `NSSTN` | label 0 | `NSSSTN` | label 1 |

#### パターン B: 2 進桁の不足

正しい値をエンコードするのに必要な桁数が不足している。

| # | ファイル | コメント | 誤 WSA | 実際の値 | 正しい WSA | 正しい値 |
|---|---------|---------|--------|---------|-----------|---------|
| 9 | `arith/mul_001.wsa` | Push 7 | `SSSTTN` | +3 (11₂) | `SSSTTTN` | +7 (111₂) |
| 10 | `arith/combined_001.wsa` | Push 4 | `SSSTSN` | +2 (10₂) | `SSSTSSN` | +4 (100₂) |

#### パターン C: 負数の桁欠落

| # | ファイル | コメント | 誤 WSA | 実際の値 | 正しい WSA | 正しい値 |
|---|---------|---------|--------|---------|-----------|---------|
| 11 | `flow/jump_if_neg_true_001.wsa` | Push -1 | `SSTN` | -0 | `SSTTN` | -1 (1₂) |

### 検証: 成功しているテストのエンコーディング

以下のテストは正しいエンコーディングでパスしている:
- `add_001.wsa`: Push 3 (`SSSTTN`) ✓, Push 5 (`SSSTSTN`) ✓
- `div_001.wsa`: Push 17 (`SSSTSSSTN`) ✓, Push 5 (`SSSTSTN`) ✓
- `mod_001.wsa`: 同上 ✓
- `jump_001.wsa`: Label 0 (`NSSSN`) ✓, Jump 0 (`NSNSN`) ✓
- `jump_if_zero_true_001.wsa`: Label 0 (`NSSSN`) ✓ 

## 原因 2: test-manifest.yaml の ws_runtime_error パス誤り（4 テスト）

### 問題

`test_ws_runtime_error_base` 関数は以下のようにパスを構築する:

```rust
let path_base = format!("resources/tests_ws/fails/runtime/{}", test_name);
```

しかし `test-manifest.yaml` では `ws_runtime_error` 型テストのパスが相対パスで指定されている:

```yaml
- name: test_ws_errors_stack_underflow_001
  type: ws_runtime_error
  path: ../fails/runtime/stack_underflow_001  # ← passes/ からの相対パス
```

これにより実際のファイルパスが
`resources/tests_ws/fails/runtime/../fails/runtime/stack_underflow_001.wsa`
→ `resources/tests_ws/fails/fails/runtime/stack_underflow_001.wsa`
となり、ファイル不存在エラーが発生する。

### 修正方法

マニフェストのパスをベース名のみに変更する:

```yaml
# 修正前
path: ../fails/runtime/stack_underflow_001

# 修正後
path: stack_underflow_001
```

対象 4 件:
- `test_ws_errors_stack_underflow_001`
- `test_ws_errors_div_zero_001`
- `test_ws_errors_callstack_underflow_001`
- `test_ws_errors_undefined_label_001`

## 修正計画

### ステップ 1: WSA ファイルの修正（8 ファイル、11 箇所）

`.wsa` ファイルの誤った数値エンコーディングを修正する。`check.json` ファイルは既に正しい期待値を持っているため変更不要。

### ステップ 2: test-manifest.yaml のパス修正（4 件）

`ws_runtime_error` 型テストのパスをベース名のみに変更する。

### ステップ 3: テスト実行による検証

`cargo test --test whitespace_direct_test` で 12 件の失敗が全て解消されることを確認する。

## 影響範囲

- `resources/tests_ws/passes/` 配下の `.wsa` ファイル 8 件
- `resources/tests_ws/test-manifest.yaml` 1 件
- ソースコード（パーサ・インタプリタ）の変更は不要

## 完了記録

- 2026-02-17: 全 11 箇所の WSA エンコーディング修正 + 4 件のマニフェストパス修正を実施
- `cargo test --test whitespace_direct_test`: 39 passed, 0 failed
- `cargo test`: 全テストスイート 0 failure
- 修正済み WSA ファイルにはエンコーディングの注意事項をコメントとして追記
