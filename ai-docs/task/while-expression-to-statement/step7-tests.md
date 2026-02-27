# Step 7: テストの更新

## 概要

既存テストの互換性確認と、必要なテスト修正・追加を行う。

## 既存テストの影響

### 後方互換性

構文 `while: cond { body };` は変更後も有効（`;` は while 文の構文の一部）。
そのため、既存の while テストコードは原則そのまま動作する。

### 影響を受ける可能性のあるテスト

| テストファイル | 影響 |
|---|---|
| `resources/tests/passes/control_flow/while_001.ns` | 互換: 動作変更なし |
| `resources/tests/passes/control_flow/while_expr_value_001.ns` | **確認必要**: テスト名に「式（expr）」を含むが、内容は式文としてのwhileなので動作は変わらない |
| `resources/tests/passes/control_flow/while_func_stack_001.ns` | 互換: 動作変更なし |
| `resources/tests/passes/control_flow/break_continue_001.ns` | 互換: 動作変更なし |
| インタプリタ Unit テスト (`src/interpreter/exec.rs`) | `test_while_loop`, `test_break_in_while`, `test_continue_in_while` - 動作変更なし |
| optimizer テスト (`src/optimizer/tests.rs`) | `test_condition_mode_zero_while`, `test_condition_mode_nonzero_while_multiple` - 動作検証必要 |

### `while_expr_value_001.ns` の扱い

このテストは while が void 型の式文として使えることをテストしている。
while が文になった後も、テスト内容（while のループ動作・break 等）自体は同じように動作する。
ただし、テスト名とコメントが「式（expr）」に言及しているため、以下のいずれかの対応を行う:

1. **テスト名・コメントを更新**: 「while 式」→「while 文」に名称変更
2. **テストをそのまま維持**: 後方互換性テストとして残す

推奨: オプション 1（名称更新）。

## 追加テスト

### while が式として使用できないことのテスト

以下のコードがコンパイルエラーになることを確認するエラーテストを追加:

```
# while を式として使用しようとするとエラー #
func: main() {
  let: x;
  x = while: 0 { };  # パースエラー #
  return: 0;
}
```

テストファイル: `resources/tests/fails/syntax/while_as_expression_001.ns`

## 確認手順

1. `cargo test` で全テストが通ること
2. large テスト（`cargo test --test code_test`）が通ること
3. Whitespace コンパイルテスト（`cargo test --test compile_test`）が通ること
