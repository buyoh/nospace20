# Whitespace Slide 命令（v0.3）対応

whitespace20（`src/whitespace/`）に Whitespace v0.3 の Slide 命令（`[Space][Tab][LF] <n>`）を追加し、v0.3 の標準プログラムを実行できるようにする。あわせて、コンパイラが Slide を使うことで効率化できるかを調査する。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

親: [code-review-2026-10 #15](../code-review-2026-10/15-ws-vm-instruction-inconsistency.md) の項目 2（解決策 B）

## 背景・動機

- `parse_stack_op`（`src/whitespace/parser.rs`）が `[S][T][L]` を `InvalidCommand` として拒否する。whitespace20 は汎用 Whitespace インタプリタとして公開されているが、Slide を使う v0.3 のプログラムを実行できない
  - 例: `push 1; push 2; push 3; slide 1; outnum; outnum` は wsc で `31` を出力し、whitespace20 では `Parse error: InvalidCommand { position: 18, imp: "Stack" }` になる（2026-10-19 確認）
- 同じ v0.3 命令の Copy は対応済みで、v0.3 対応が中途半端になっている

親ドキュメントの NOTE にある導入手続きの確認結果:

| 手続き | 結果 |
|---|---|
| `docs/spec-whitespace.md` に v0.3 の命令を追記 | 記載済み（スタック操作表の `[Tab][LF]` 行）。追記不要 |
| 拡張命令を阻害しないか | 阻害しない。`docs-ai/architecture/whitespace-runtime.md` の拡張スロット表は `[S][T][L]` を最初から「slide（使用済）」として除外しており、拡張可能スロット一覧（7 個）は変わらない。拡張 API は負ヒープアドレス方式で命令スロットを使わない |
| コンパイラで効率的なコード生成が可能か | [02-compiler-usage-study.md](02-compiler-usage-study.md) で調査する（本タスクでは VM 対応と分離） |

## 設計・方針

- `base::ws_types::Instruction` に `Slide(WsNumber)` を追加し、パーサ・エンコーダ・ニーモニック・VM・プロファイラを対応させる（詳細: [01-parser-vm-support.md](01-parser-vm-support.md)）
- 負の `n` は `StackUnderflow` とし、将来の独自拡張のために予約する（wsc と同じ結果）。Copy（[ws-vm-copy-index-conversion.md](../ws-vm-copy-index-conversion.md)）は既存挙動維持のため `|n|` だが、新命令の Slide には互換性の制約がないため揃えない。**この点はユーザー確認待ち**（詳細は [01](01-parser-vm-support.md)「命令の意味」）
- スタック深さからインデックスへの変換は Copy タスクで追加する `index_from_top` を共用する。**Copy タスクを先に実装する**
- コンパイラ側での Slide 利用は調査のみ。採用する場合は別タスクとして起票する

## ドキュメント

| # | ドキュメント | 内容 |
|---|---|---|
| 01 | [01-parser-vm-support.md](01-parser-vm-support.md) | `Instruction::Slide` 追加、パーサ・VM・プロファイラ・テストの変更 |
| 02 | [02-compiler-usage-study.md](02-compiler-usage-study.md) | コンパイラが Slide を使うことで効率化できる箇所の調査計画 |

## タスクステップ・チェックリスト

1. [ ] 前提: [ws-vm-copy-index-conversion.md](../ws-vm-copy-index-conversion.md) の実装完了（`index_from_top` が存在すること）
2. [ ] [01-parser-vm-support.md](01-parser-vm-support.md) の実装
3. [ ] [02-compiler-usage-study.md](02-compiler-usage-study.md) の調査と結果の記録
4. [ ] 親ドキュメント（#15）の進捗を更新
