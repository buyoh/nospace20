# test_example_qsort_ws_self 失敗調査

## 概要

Fix A および Fix B の実装により、5件の失敗テストのうち4件が解決したが、
`test_example_qsort_ws_self` のみが依然として失敗している。

## 現在の状況

**テスト名**: test_example_qsort_ws_self  
**テストパス**: examples/e1-00-qsort  
**失敗パターン**: 出力不一致  
**期待される出力**: "1 1 2 3 4 5 9 "  
**実際の出力**: ""（空）

## 原因特定済み (2026-02-17)

**Bug C: while ループ本体のブロック値がスタックに蓄積する問題**

`generate_while_expression` がループ本体に `generate_block` を使用しているが、
`generate_block` が末尾で生成する `push 0`（ブロック式値）がイテレーションごとに
スタックに蓄積し、`generate_local_deallocate` でのスタック不整合を引き起こしていた。

詳細な分析と修正設計は [fix-while-loop-stack-leak.md](../done-task/fix-while-loop-stack-leak.md) を参照。

## Fix C 実装後の状況 (2026-02-17)

Fix C を実装した結果、出力が変化:
- 修正前: "" (空)
- 修正後: "0 0 0 1 1 4 7 " (不正確だが空ではない)
- 期待値: "1 1 2 3 4 5 9 "

部分的な改善が見られるが、依然として正しい出力は得られていない。
他の 114/115 の ws_self テストは成功している。

**現在の仮説**: qsort 特有の複雑な再帰構造や配列操作に関連する別のバグが存在する可能性。
さらなる調査が必要。

## Bug D 特定 (2026-02-17)

**Bug D: ブロックスコープ変数のヒープオフセット衝突**

`CodeGenContext::get_var_info` が `IdentifierRef.scope_depth` を無視し、
`local_index` をそのまま `offset` として使用するため、内部ブロックスコープの変数が
関数スコープの変数とヒープアドレスを共有する。

main() の内部ブロック `{ let: i(0); ... }` の `i` が `arr[0]` と同じ `heap[LHB+0]` に配置され、
配列データが破壊される。

修正設計: [fix-block-scope-offset/](fix-block-scope-offset/)

## スコープテスト追加による失敗テスト拡大と修正経過 (2026-02-17)

ブロックスコープ関連のテストが追加された結果、一時的に18件まで失敗が増加したが、
Bug D (ブロックスコープオフセット衝突）と static 変数関連の修正により14件が解消された。

### 失敗分類 (当時の18件)

| 根本原因 | 件数 | 状態 |
|----------|------|------|
| **Bug D: ブロックスコープオフセット衝突** | 11件 | ✅ 修正済み |
| **ブロック/if/while 式の値返却未実装** | 2件 | ❌ 未修正 (Bug E) |
| **Bug D + 式の値返却の複合** | 2件 | ⚠️ Bug D 部分は修正、値返却は未修正 (Bug E) |
| **static 変数 + ネスト関数のスコープ** | 3件 | ✅ 修正済み |

### 残り 4件の失敗テスト (Bug E)

| テスト名 | 説明 |
|----------|------|
| test_control_flow_if_expr_value_001_ws_self | `x = if: 1 { 5; } else: { 10; };` |
| test_control_flow_while_expr_value_001_ws_self | `x = while: ... { i; };` |
| test_scope_block_expr_nested_001_ws_self | `result = { let:a; ... { let:b; a+b; }; };` |
| test_scope_block_expr_value_001_ws_self | `x = { let:a; a=3; a; };` |

## Bug E 特定 (2026-02-17): ブロック/if/while 式の値返却未実装

### 現在の状況

Bug D のブロックスコープオフセット衝突は修正済み。残りの失敗テストは **4件** のみ。

```
test_control_flow_if_expr_value_001_ws_self      ... FAILED (AssertionFailed(0))
test_control_flow_while_expr_value_001_ws_self    ... FAILED (AssertionFailed(0))
test_scope_block_expr_nested_001_ws_self          ... FAILED (AssertionFailed(0))
test_scope_block_expr_value_001_ws_self           ... FAILED (AssertionFailed(0))
```

ws_self 全体: 117 passed, 4 failed

インタプリタ版テスト (suffix なし) は 4件すべて PASS。

### 根本原因

**`generate_block` がブロック内の最後の式の値を無視し、常に `push 0` を返している。**

#### コンパイラの問題箇所 (2つのレイヤー)

1. **`generate_statement` (statement.rs L61-L64)**:
   `ExecStatement::Expression` で式の評価結果を常に `Discard` で捨てる。
   → ブロック内の最後の式の値も破棄される。

2. **`generate_block` (statement.rs L37-L53)**:
   全ての文を `generate_statement` で処理した後、無条件に `push 0` する。
   → ブロック式の値は常に 0。

#### 影響の連鎖

- `generate_if_expression` (expression.rs): then/else の各ブロックを `generate_block` で生成するため、if 式の値も常に 0。
- `generate_while_expression` (expression.rs): ループ本体を `generate_block` で生成し、その結果を `Discard` で捨て、ループ終了後に `push 0` するため、while 式の値も常に 0。
- `ExecExpression::Block` (expression.rs L77-L79): `generate_block` をそのまま呼ぶため、ブロック式の値も常に 0。

#### 正しい動作 (インタプリタ)

インタプリタの `interpret_statements_with_value` は各式文の値を `last_value` に蓄積し、ブロックの最後の式の値を返す:
- `interpret_block`: `interpret_statements_with_value` の結果をブロック値として返す
- `interpret_if`: 選択されたブロックの `interpret_statements_with_value` 結果を返す
- `interpret_while`: ループ最後のイテレーションの `last_value` を返す (break 時は 0)

### 各テストの具体的な失敗パターン

#### test_scope_block_expr_value_001_ws_self
```nospace
x = { let: a; a = 3; a; };
__assert(x == 3);  # x は 0 になるため AssertionFailed #
```
`generate_block` が `a;` (値=3) の結果を Discard して `push 0` → `x = 0`

#### test_scope_block_expr_nested_001_ws_self
```nospace
result = { let: a; a = 10; { let: b; b = 5; a + b; }; };
__assert(result == 15);  # result は 0 になるため AssertionFailed #
```
内側ブロックの `a + b` (=15) が Discard され内側 `push 0`、外側もそれを Discard して `push 0` → `result = 0`

#### test_control_flow_if_expr_value_001_ws_self
```nospace
x = if: 1 { 5; } else: { 10; };
__assert(x == 5);  # x は 0 になるため AssertionFailed #
```
then ブロックの `5;` が Discard され `generate_block` が `push 0` → `x = 0`

#### test_control_flow_while_expr_value_001_ws_self
```nospace
x = while: i - 3 { i = i + 1; i; };
__assert(x == 3);  # x は 0 になるため AssertionFailed #
```
ループ本体の `i;` が Discard され `generate_block` が `push 0`、それも Discard されてループ終了後 `push 0` → `x = 0`

### 仕様との関係

| 式の種類 | spec.md | インタプリタ | compiler_ws (現状) |
|----------|---------|-------------|-------------------|
| ブロック式 (§6.5) | 最後の式の値を返す | ✅ 正しく実装 | ❌ 常に 0 |
| if 式 (§6.2) | 常に 0 (TODO) | ✅ 最後の式の値を返す | ❌ 常に 0 |
| while 式 (§6.1) | 常に 0 (TODO) | ✅ 最後の式の値を返す | ❌ 常に 0 |

- **ブロック式**: spec.md §6.5 は「最後に評価された式の値」と明記。コンパイラの修正が必須。
- **if / while 式**: spec.md §6.1/§6.2 は「常に 0」だが TODO が付いており、インタプリタは既に値返却を実装済み。テストケースも値返却を期待している。

### 修正方針案

`generate_block` を修正し、ブロック内の最後の式の値をスタックに残す:

1. **ブロック内の最後の文が式文の場合**: `Discard` を付けずに値をスタックに残す
2. **それ以外の文**: 従来通り `Discard` する
3. **ブロックが空の場合**: `push 0` を返す (仕様通り)
4. **最後の文が return/break/continue の場合**: 値は使われないが、スタック整合性のため `push 0` が必要

具体的な変更箇所:
- `statement.rs`: `generate_block` を「最後の文を特別扱い」するロジックに変更
- `statement.rs`: `generate_statement` に「値を残すモード」を追加するか、最後の文だけ直接 `generate_expression` を呼ぶ
- `expression.rs`: `generate_while_expression` でループ本体の値を `last_value` 変数的に管理する設計が必要（while 式の値返却には追加のヒープ変数が必要な可能性あり）

### 修正の複雑度

- **ブロック式・if 式**: 比較的単純。`generate_block` の最後の文のみ `Discard` を省略すればよい。
- **while 式**: やや複雑。ループの各イテレーションで値を保持する必要がある。break 時は 0 を返す仕様。Whitespace のスタックマシンでは「前回イテレーションの値を保持」するために一時変数（ヒープ）の使用が必要。

## 関連ドキュメント

- [fix-block-scope-offset/](fix-block-scope-offset/) - Bug D 修正設計
- [fix-while-loop-stack-leak.md](../done-task/fix-while-loop-stack-leak.md) - 修正設計（Fix C）
- [remaining-ws-self-failures.md](../done-task/remaining-ws-self-failures.md) - Fix A/B の実装記録
- [whitespace-self-test-failures.md](whitespace-self-test-failures.md) - 全体の失敗テスト管理
