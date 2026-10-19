# 03: パーサのネスト深度上限・NospaceVM フレーム数上限・CLI run の NospaceVM 移行

作成日: 2026-10-19
ステータス: 完了

## 背景・動機

[07-recursion-depth-limit.md](../code-review-2026-10/07-recursion-depth-limit.md) 参照。採用案 A + C + E。

調査で判明した点: スタックを消費するのはパーサの再帰だけでなく、意味解析・コンパイラ・最適化の AST 再帰である。`1+1+...+1`（300 項）はパーサではループだが左深の AST を作り、debug ビルドの意味解析が 27KB/段のフレームで main スレッド（8MB）を使い切る（release は 3000 項で正常、10000 項で abort）。したがって上限は「パーサの再帰深度」ではなく「生成される AST の深さ」に対して掛ける。

## 設計・方針

### A: ネスト深度上限（`tree_parser`）

- `MAX_NESTING_DEPTH: usize = 128`（`tree_parser/mod.rs`。当初 256 としたが実測により変更。README の進捗参照）
- `ExpressionBuilder` / `StatementBuilder` に `depth`（現在のネスト深さ）と `peak`（到達した最大深さ）を持たせる。builder 間の呼び出し（`parse_to_expression_tree_at` / `parse_to_statements_at`）では `depth` を渡し、`peak` を返して引き継ぐ
- `nested(levels, f)`: 深さを `levels` 段増やして `f` を実行。上限超過時はエラーを記録し、`f` を実行しない（トークンも消費しない）
  - 式の root（括弧・引数・添字・if 条件などの部分式はすべてここを経由）: 1
  - 単項演算子の並び: 演算子の個数
  - 右結合の代入の連鎖: 1 段ごとに 1
  - ブロック本体・if 本体・while 本体（`parse_nested_statements`）: 1
  - `else: if:` の連鎖: 1 段ごとに 2
- 左結合の二項演算子の連鎖（`parse_left_assoc_chain`）: 左右のオペランドの高さを `measured` で計測し、各ノードの高さを `max(左, 右) + 1` として `depth + 高さ` が上限を超えたらエラー。計測した高さを `peak` に反映する
- 上限超過時: `"nesting too deep (limit: 128)"` のパースエラーを記録し `Invalid` を返す（01 のタスクで導入した引数ループの前進チェック・文パーサの回復で終了する）
- 1 つの builder 内で深さ超過エラーを 1 回だけ記録するよう抑制はしない（CLI は先頭 3 件のみ表示）

### C: CLI `--mode run` を NospaceVM で実行

- `bin/nospace20.rs`: `NospaceVM::from_scope(a)` → `with_config`（`ignore_debug`）→ `with_io(stdin, stdout)` → `step(STEP_BUDGET)` ループ
- `Complete { Some(v) }` → `main returns: v`、`None` → `main exited`、`Error(e)` → `error: {e}` で exit 1。完了後 `flush()`（失敗は exit 1）
- `--debug` の trace 表示は `vm.traced()`
- 深い再帰（3000 段など）が abort せずに実行できる

### E: NospaceVM のフレーム数上限

- `EnvironmentConfig::max_vm_frames: Option<usize>`。既定値 `Some(DEFAULT_MAX_VM_FRAMES)`
- `DEFAULT_MAX_VM_FRAMES = 1_000_000`（当初案 4,000,000 から実測で変更。`Frame` は 88 バイト）
- `execute_one_step` の後でフレーム数を確認し、超過で `InterpretError::StackOverflow(limit)`

## テスト

- パーサのユニットテスト: 括弧・単項・二項連鎖・ブロックの各ネストが上限で `nesting too deep`、上限未満は成功
- large テスト `fails/syntax/nesting_too_deep_{paren,unary,binary,block}_001`（深さ数千。ハングしないことと終了することの回帰）
- VM のユニットテスト: 深い再帰（例: 10 万段）が完了する、フレーム上限を小さく設定すると `StackOverflow`
- CLI: 手動確認（深い再帰、`| head` での BrokenPipe）

## チェックリスト

1. [x] A: ネスト深度上限とテスト
2. [x] E: VM フレーム数上限とテスト
3. [x] C: CLI の NospaceVM 移行
4. [x] spec.md（Appendix に処理系の上限）、VERSION.md、README.md（必要なら）
