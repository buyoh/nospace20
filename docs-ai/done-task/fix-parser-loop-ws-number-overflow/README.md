# パーサ無限ループ・WS パーサ数値/ラベル・WS VM 算術オーバーフローの修正

コードレビュー 2026-10 の high 指摘のうち 01 / 03 / 04 をまとめて修正するための設計。いずれも「信頼できない入力でハング・panic(abort) する」問題で、修正範囲が互いに独立しているため 1 タスクとしてまとめる。

作成日: 2026-10-18
ステータス: 完了

## 背景・動機

指摘内容・再現手順は以下のレビュー記録を参照（本タスクでは繰り返さない）。

| 本タスク | レビュー指摘 | 採用案 |
|---|---|---|
| [01-parser-arg-loop-progress.md](01-parser-arg-loop-progress.md) | [code-review-2026-10/01](../../task/code-review-2026-10/01-parser-infinite-loop.md) 引数リストで無限ループ | 案 B（引数ループで前進チェック） |
| [02-ws-parser-number-and-label.md](02-ws-parser-number-and-label.md) | [code-review-2026-10/04](../../task/code-review-2026-10/04-ws-parser-number-label.md) WS パーサの数値オーバーフロー・`i64::MIN`・ラベル衝突 | 案 A + B + C |
| [03-ws-vm-wrapping-arithmetic.md](03-ws-vm-wrapping-arithmetic.md) | [code-review-2026-10/03](../../task/code-review-2026-10/03-ws-vm-arithmetic-overflow.md) WS VM の算術オーバーフロー panic | 案 A（wrapping 統一）+ spec.md 記載（言語仕様は「実装依存」、処理系の挙動は Appendix） |

WASM の `parse()` / `compile()` / `WasmWhitespaceVM::new` は同期処理で、`Cargo.toml` は `panic = "abort"`。そのため 01 はブラウザタブのフリーズ、03・04 は WASM インスタンスの trap に直結する。

## 設計・方針

- 共通方針: 想定される不正入力（ソースコード・ws テキスト・実行中の値）はすべて戻り値（エラー値 / 定義済みの演算結果）で扱い、panic しない（AGENTS.md のエラーハンドリング方針）
- 各修正箇所には、バグの発生原因と解決策をソースコードコメントとして残す（AGENTS.md）
- 各修正にユニットテストと large テストを追加する
- コード変更ごとに `VERSION.md` の WIP セクションへ変更内容を追記する（AGENTS.md。docs-ai の更新・テスト追加は記載しない）
- 公開 API（`tree_parser::parse_to_tree`、`whitespace::parse`、`WhitespaceVM::from_source` / `from_instructions`）のシグネチャは変更しない
- 本タスクの範囲外とするもの
  - 引数ループの共通化（レビュー 01 の案 C / [17-tree-parser-duplication.md](../../task/code-review-2026-10/17-tree-parser-duplication.md)）。01 で追加する前進チェックは共通化時にそのまま移設できる形にしておく
  - condition_opt のオーバーフロー時の意味変化（[09](../../task/code-review-2026-10/09-condition-opt-overflow-semantics.md)）。03 で本処理系の挙動を「wrapping」と確定させる（言語仕様上は実装依存）ので、09 はその挙動に対して修正する
  - wsc（多倍長整数）との挙動差の解消。言語仕様上、整数の範囲とオーバーフロー時の挙動は実装依存とし、本処理系の挙動と外部処理系との差異は spec.md の Appendix に記載するに留める

## タスクステップ・チェックリスト

各ステップは独立しており、どの順で実施してもよい。02 の large テスト（`--opt all` で `i64::MIN` を push するケース）は 03 と無関係に成立する。

1. [x] 01: 引数ループの前進保証（[01-parser-arg-loop-progress.md](01-parser-arg-loop-progress.md)）
2. [x] 02: WS パーサの数値 checked 化・ラベルのビット列化・境界値テスト（[02-ws-parser-number-and-label.md](02-ws-parser-number-and-label.md)）
3. [x] 03: WS VM 算術演算の wrapping 統一・spec.md（int 型の実装依存の明記、Appendix に処理系の挙動）（[03-ws-vm-wrapping-arithmetic.md](03-ws-vm-wrapping-arithmetic.md)）
4. [x] `cargo test` 全体が通ることを確認（wsc を使う `#[ignore]` テストは `./tools/setup-wsc.sh` 済みなら `cargo test -- --ignored` でも確認）
5. [x] [code-review-2026-10/README.md](../../task/code-review-2026-10/README.md) の 01 / 03 / 04 の状態を「完了」に更新し、本ディレクトリを done-task に移動

## 進捗

### 2026-10-19

- 01 / 02 / 03 を実装。`cargo test` 全件成功（wsc クロスバリデーションは `cargo test --test whitespace_direct_test -- --include-ignored` で新規 3 件を含め成功、nospace の新規 large テストも `--include-ignored` で wsc ターゲット含め成功）
- 設計からの変更点
  - 02: `label_interner_tests.rs` は作成しない。AGENTS.md の更新（新規モジュールの Unit テストは、複数の自明でないモジュールが関連する場合のみ）に従い、`LabelInterner` 単体は自明なモジュールとして扱い、ラベルの区別は `parser_tests.rs` / `interpreter_tests.rs` で検証する
  - 02 / 03: nospace の large テストは `resources/tests/passes/` 直下ではなく `resources/tests/passes/operators/` に配置（既存の演算子テストと同じ場所）
  - 03: large テスト `int_overflow_wrapping_001` に単項 `-`（`-min`）のケースを追加
- 01 の large テストで、文パーサ側も全再現ケース（`f(;` `f(]);` `f(}` `f([2,3]);` `f(:` `struct: S(;)`、および `f(1:)`）で終了することを確認
- コードレビュー（1 回）: critical / major なし。minor 7 件のうち 6 件に対応
  - 対応: stall 時のエラー再利用を「引数解析中に記録されたエラー」に限定、`f(1 ;` のユニットテスト追加、spec.md Appendix の CLI 表記修正、tests_ws README の `InvalidNumber` を未使用と明記、`InvalidLabel` に使用中である旨のコメント、レビュー記録の状態更新
  - 未対応: whitespace20 がパースエラーを `{:?}` で表示し、`NumberOverflow` の Display メッセージが使われない点 → CLI エラー出力の既存指摘 [16-cli-error-output.md](../../task/code-review-2026-10/16-cli-error-output.md) に追記
- done-task に移動
