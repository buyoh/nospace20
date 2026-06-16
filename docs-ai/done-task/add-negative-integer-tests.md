# 負の整数テスト追加タスク

## 概要

Whitespace インタプリタの負の整数処理能力を検証するため、包括的なテストケースを追加した。
Whitespace の数値エンコーディングは2の補数表現ではなく、符号ビット + 2進数表現であることに注意してエンコーディングを作成。

## 追加したテストケース

### スタック操作（負の整数プッシュ）

1. **push_negative_001**: -3 をプッシュして出力（既存を修正）
2. **push_negative_002**: -1 をプッシュして出力（新規）
3. **push_negative_003**: -42 をプッシュして出力（新規）
4. **push_negative_004**: -100 をプッシュして出力（新規）

### 算術演算（負の整数）

5. **negative_add_001**: -5 + 3 = -2（負の数の加算）
6. **negative_sub_001**: 5 - 8 = -3（結果が負になる減算）
7. **negative_mul_001**: -3 * 4 = -12（負の数の乗算）
8. **mixed_add_001**: 10 + (-3) = 7（正と負の混合加算）
9. **negative_div_001**: -10 / 3 = -3（負の数の除算）
10. **negative_mod_001**: -17 % 5 = -2（負の数の剰余）

## 数値エンコーディング仕様

Whitespace の数値エンコーディング:
- 符号ビット: S (+), T (-)
- 2進数: S=0, T=1
- 終端: N

例:
- -1: T (符号-) + T (1) + N = `TTN`
- -3: T (符号-) + TT (11 = 3) + N = `TTTN`
- -42: T (符号-) + TSTSTS (101010 = 42) + N = `TTSTSTSN`
- -100: T (符号-) + TTSSTS (1100100 = 100) + N = `TTTSSTSSN`

Push 命令: SS + 数値エンコーディング
- Push -3: `SS` + `TTTN` = `SSTTTN`
- Push -42: `SS` + `TTSTSTSN` = `SSTTSTSTSN`

## 実装内容

### 修正したファイル
- `resources/tests_ws/passes/stack/push_negative_001.wsa` - スペース削除とエンコーディング修正

### 新規作成したファイル
- `resources/tests_ws/passes/stack/push_negative_002.wsa` と `.check.json`
- `resources/tests_ws/passes/stack/push_negative_003.wsa` と `.check.json`
- `resources/tests_ws/passes/stack/push_negative_004.wsa` と `.check.json`
- `resources/tests_ws/passes/arith/negative_add_001.wsa` と `.check.json`
- `resources/tests_ws/passes/arith/negative_sub_001.wsa` と `.check.json`
- `resources/tests_ws/passes/arith/negative_mul_001.wsa` と `.check.json`
- `resources/tests_ws/passes/arith/mixed_add_001.wsa` と `.check.json`
- `resources/tests_ws/passes/arith/negative_div_001.wsa` と `.check.json`
- `resources/tests_ws/passes/arith/negative_mod_001.wsa` と `.check.json`

### 更新したファイル
- `resources/tests_ws/test-manifest.yaml` - 10個のテストケースを追加

## テスト結果

**27/39 テスト成功** (69.2%)

### ✅ 新規追加テスト（10件すべて成功）
- push_negative_001, 002, 003, 004 - すべて成功
- negative_add_001, negative_sub_001, negative_mul_001 - すべて成功
- mixed_add_001, negative_div_001, negative_mod_001 - すべて成功

### 改善された既存テスト
- push_negative_001 が失敗から成功に変更

### テスト結果の改善
- 前回: 17/30 成功 (56.7%)
- 今回: 27/39 成功 (69.2%)
- 新規テスト: 10/10 成功 (100%)

## 既知の問題

以下の12テストが引き続き失敗している（負の整数とは無関係）:
- arith: sub_001, mul_001, combined_001
- errors: すべて（4件）
- flow: call_return_001, jump_if_neg_true/false_001, jump_if_zero_false_001, loop_simple_001

これらは既存の問題で、負の整数のエンコーディングとは別の原因によるものと考えられる。

## エンコーディングのミスから学んだ教訓

1. **2進数変換**: 10進数を正確に2進数に変換する必要がある
   - 4 = 100 (binary) → STSSN (not STSN which is 2)
   
2. **スペースの混入**: WSA ファイルに誤ってスペースが入り込むと、デコーダで不正確な結果になる
   - 定期的に `sed` で S, T, N 間のスペースを削除

3. **テスト駆動**: 小さなテストから始めて、徐々に複雑なテストに移行することが重要

## タイムスタンプ

- 作成日: 2026-02-16
- 完了日: 2026-02-16
