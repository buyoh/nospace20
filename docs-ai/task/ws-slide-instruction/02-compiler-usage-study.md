# 02: コンパイラでの Slide 利用の調査

`compiler_ws` / peephole が Slide を出力することで、命令数・実行ステップ数を減らせる箇所があるかを調査する。本ドキュメントは調査計画であり、コンパイラの変更は行わない。

作成日: 2026-10-19
ステータス: 未着手

## 背景・動機

親ドキュメント #15 の NOTE「コンパイラがこれらの命令を使うことでより効率的なコード生成が可能になるか検討」に対応する。
Slide は「先頭を残して下の n 個を捨てる」操作を 1 命令で行えるため、`Swap; Discard` の繰り返しや、値を一時的にヒープ（`TEMP_PTR`）へ退避して捨てる処理を置き換えられる可能性がある。

一方、コンパイラの出力に Slide を含めると、v0.3 未対応の Whitespace 処理系では出力が実行できなくなる（Copy は既に `alloc_runtime/` で使用しているため、v0.3 依存自体は既に存在する）。

## 設計・方針

### 調査項目

| # | 対象 | 観点 |
|---|---|---|
| 1 | `src/compiler_ws/` 全体 | `Swap` 直後に `Discard` が続く命令列（`[..., a, b] -> [..., b]` は `Slide(1)` と等価）を列挙 |
| 2 | `builtin.rs` の比較ルーチン（`generate_comparator_zero` / `generate_comparator_negative`） | 分岐後の `Swap` / `Discard` を Slide で短縮できるか |
| 3 | `statement.rs` の `generate_return` / `generate_void_return` と `alloc_runtime` の関数エピローグ | 戻り値を残して局所的な値を捨てる処理に使えるか |
| 4 | `alloc_runtime/`（bump, fsba） | 作業用の値を捨てる `Discard` の連続を `Slide(n)` にまとめられるか |
| 5 | 式評価で引数を評価して捨てる箇所（例: `expression.rs` の残り引数の破棄） | 値の並びと Slide の適用可否 |

### 評価方法

- `cargo run --example ws_profiler` で `resources/tests/profile-targets.yaml` の対象を計測し、`total_steps` と `program.instruction_count`（静的命令数）を変更前後で比較する（手順は `docs/profiler.md`）
- 候補ごとに「静的命令数の削減」「実行ステップ数の削減」「wsc で実行できるか」を表にまとめる

### 判断基準

- 実行ステップ数または静的命令数が目に見えて減る候補があれば、peephole パス（`src/compiler_ws/peephole.rs`）のルール追加またはコード生成の変更として別タスクを起票する
- 効果が小さい場合は「採用しない」と結論を記録して終了する

## タスクステップ・チェックリスト

1. [ ] 前提: [01-parser-vm-support.md](01-parser-vm-support.md) の完了（VM で Slide を実行できること）
2. [ ] 調査項目 1〜5 の候補を列挙し、本ドキュメントに追記
3. [ ] 有望な候補について試作し、ws_profiler で効果を計測（試作コードはコミットしない）
4. [ ] 結論（採用 / 不採用、採用時の起票先）を本ドキュメントに記録
