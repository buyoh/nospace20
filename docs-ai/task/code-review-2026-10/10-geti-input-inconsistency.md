# `__geti` の入力解釈がインタプリタ・WS VM・言語仕様の三者で食い違う

- 重要度: **medium**
- 状態: 未着手

## 進捗

- [ ] 仕様の確定（区切り・EOF・不正入力）
- [ ] 共通パーサの実装
- [ ] io テスト追加

## 問題点

- `src/interpreter/environment.rs:150-191`（`read_int`）
- `src/whitespace/interpreter.rs:933-952`（`read_number`）
- 仕様 `docs/spec.md:961`「`__geti` は空白・改行をスキップして数値を読み取る」

### 再現: `a = __geti(); b = __geti();` を両方出力

| 入力 | nospace20 run | WS コンパイル後（whitespace20） |
|---|---|---|
| `"1 2\n3\n"` | `1, 3`（2 が行ごと捨てられる） | `I/O error: invalid digit` |
| `"x\n5\n6\n"` | `5, 6`（数字のない行を黙ってスキップ） | エラー |
| `"7\n"` | `7, 0`（EOF を 0 とする） | `cannot parse integer from empty string` |

インタプリタでは `"-9223372036854775808"` が parse 失敗で 0 になる（`:187-190`）。

## 違反している原則

- 仕様準拠、バックエンド間の一貫性
- DRY: 同じ入力処理を 3 箇所（Environment / WS VM Buffered / WS VM Interactive）で別実装

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | 「空白区切りトークンを 1 つ読む」純関数を `base/` に作り、両 VM から利用。行の残りはバッファに保持 | `environment.rs`、`whitespace/interpreter.rs`（`InteractiveBuffer` 含む） |
| B | 仕様を「1 行 = 1 整数」に改め、インタプリタを WS VM に合わせる（wsc 互換） | `docs/spec.md`、`environment.rs`、io 系テスト |
| C | EOF・不正入力の扱い（0 かエラーか）を仕様で決め、`success_io` テストに境界ケース追加 | `docs/spec.md`、テスト |

推奨: B + C（WS の `readnum` 命令は行単位であり、wsc との互換を優先）。
