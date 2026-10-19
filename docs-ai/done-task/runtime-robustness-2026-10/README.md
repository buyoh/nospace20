# 実行時エラーの Result 化・ネスト深度上限・CLI の NospaceVM 移行・WASM 最適化パス統一

コードレビュー 2026-10 の 05 / 07 / 12 をまとめて実装する。いずれも「ユーザー入力でホスト（CLI / WASM）が panic・abort する」「CLI と WASM で挙動が異なる」問題。

作成日: 2026-10-19
ステータス: 完了

## 背景・動機

| 本タスク | レビュー指摘 | 採用案 |
|---|---|---|
| [01-wasm-opt-pass-names.md](01-wasm-opt-pass-names.md) | [12-wasm-compile-opt-mismatch.md](../code-review-2026-10/12-wasm-compile-opt-mismatch.md) | A + B（推奨どおり） |
| [02-nospace-vm-runtime-errors.md](02-nospace-vm-runtime-errors.md) | [05-runtime-error-panic.md](../code-review-2026-10/05-runtime-error-panic.md) | C + B。A（ツリー走査インタプリタの Result 化）は 07-C により CLI から外れるため対象外 |
| [03-nesting-depth-and-cli-vm.md](03-nesting-depth-and-cli-vm.md) | [07-recursion-depth-limit.md](../code-review-2026-10/07-recursion-depth-limit.md) | A + C + E（ユーザー指示「C も対応する」、推奨どおり） |

## 設計・方針（共通）

- CLI の `--mode run` は NospaceVM で実行する（07-C）。ツリー走査インタプリタ（`interpreter/exec.rs`）は `lib.rs` の `interpret_*` API とテストの `interpreter` ターゲットからのみ使われる状態になる。廃止は別タスク
- NospaceVM の実行時エラーはすべて `StepResult::Error(InterpretError)` で返す。`InterpretError` にバリアントを追加する
- アロケータ・`Environment` の I/O 等は `Result<_, InterpretError>` を返す API に変更する。`exec.rs` は呼び出し側で明示的に panic する（従来どおりのメッセージ `runtime error: ...`）。既存の `interpreter` ターゲットのテストは変更しない
- 言語仕様と処理系の挙動の区別は前タスクと同じ方針（言語仕様は実装依存、本処理系の挙動は spec.md の Appendix）

## タスクステップ・チェックリスト

1. [x] 01: WASM 最適化パスの統一
2. [x] 02: NospaceVM 実行時エラーの Result 化、`__alloc` サイズ検証
3. [x] 03: パーサのネスト深度上限、NospaceVM フレーム数上限、CLI run の NospaceVM 移行
4. [x] テスト全体、VERSION.md、spec.md、README.md（CLI の説明に変更があれば）
5. [x] レビュー、done-task へ移動

## 範囲外（別タスク）

- ツリー走査インタプリタ（`exec.rs`）の廃止または Result 化（05-A）
- `WasmWhitespaceVM::new` が最適化パスを受け取らない点（12 の問題点に記載、案には含まれない）
- semantic_analyzer の式変換関数（`convert_to_exec_expression_with_resolver`、約 580 行）の debug ビルドでのスタックフレーム肥大（約 27KB/段。release は約 1〜2KB/段）。ネスト深度上限で release / WASM は保護されるが、debug ビルドでは深さ 300 程度の演算子連鎖で main スレッドのスタックを使い切る

## 進捗

### 2026-10-19

- 01 / 02 / 03 を実装。`cargo test` 全件成功、`cargo check --features wasm --target wasm32-unknown-unknown` 成功
- 設計からの変更・追加
  - 03-A: ネスト深度は「再帰の深さ」ではなく「構文木の高さ」で数えるよう変更。長い連鎖の左端のオペランドに深い部分式がある形（例: 各段の括弧が 20 項の連鎖の左端、を 20 段）で構文木の深さが積になり、単純な数え方では上限をすり抜けて abort したため。各 builder が到達した最大深さ（`peak`）を保持し、二項演算子の連鎖は `max(左の高さ, 右の高さ) + 1` で計測する。式パーサ ↔ 文パーサ間でも `peak` を受け渡す
  - 03-A: 上限を 256 → **128** に変更。release ビルドを 1MiB スタック（wasm32 の既定）で実測し、`if` のネスト約 80 段・`else: if:` の連鎖約 100 で意味解析がスタックを使い切ったため。`else: if:` は構文木に 2 段積まれるので 2 として数える
  - 03-A: 5 つの二項演算子関数の重複ループを `parse_left_assoc_chain` に統合（17 の重複指摘の一部も解消）
  - 03-A: `while` 本体が `parse_to_statements`（深さ 0 から）を呼んでいた箇所を `parse_nested_statements` に統一
  - 03-C: return の無い `__main` の戻り値が NospaceVM（最後の文の値）とツリー走査インタプリタ（None）で異なっていたため、NospaceVM を None に統一（CLI の `main exited` 表示を維持するため）
  - 03-E: 既定のフレーム数上限は 1,000,000（`Frame` は 88 バイト。再帰 40 万段で約 117MB、無限再帰は約 200MB で停止）
  - 02: `NospaceVM::flush()` は `Result` を返すよう変更（WASM の `flushStdout` はメモリ上のバッファなので結果を無視）
- 確認
  - CLI: 深い再帰（10 万〜40 万段）が完了、無限再帰は `error: stack overflow`、0 除算・assert は `error: ...` で終了コード 1、`| head` の BrokenPipe は `error: I/O error` で終了コード 1（従来は panic）
  - ネスト: 括弧 20000・単項 200000・連鎖 100000・ブロック/while 50000・代入連鎖・`else: if:` 連鎖・関数呼び出しのネスト・連鎖×括弧の積の形がすべて `nesting too deep`。上限付近の入力は release・1MiB スタックで run / `--opt all` / compile が成功
- コードレビュー 2 回
  - 1 回目: critical 1（後置演算子の連鎖がネスト上限をすり抜ける）、major 3（型指定の深さ、`repeat:` の脱糖による深さ、エラー後に VM が実行を再開する）、minor 7。critical / major と minor 5 件（CLI の出力 panic・flush によるエラーの隠蔽・添字のオーバーフロー・VM の `max_expression_count`・テスト不足）に対応
  - 2 回目: 1 回目の修正をすべて確認。新規 major 1（本タスク以前からの不具合: 代入できない左辺が意味解析を通過し VM が panic）は VM 側を `InterpretError::NotAssignable` に変更して abort を防ぎ、意味解析での検査は残課題へ。minor（WASM に失敗状態の公開がない）は `isFailed()` を追加
  - 残課題（ヒープ総量上限、`NestingTracker` 抽出、1MiB スタックの自動確認、意味解析関数の分割、`exec.rs` 廃止、左辺値の意味解析検査ほか）は [runtime-robustness-followups.md](../../task/runtime-robustness-followups.md) に記録
- done-task に移動
