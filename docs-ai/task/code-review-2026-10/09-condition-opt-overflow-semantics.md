# 最適化: condition_opt と比較演算のコード生成がオーバーフロー時に意味を変える

- 重要度: **medium**
- 状態: 未着手

## 進捗

- [ ] 方針決定（[03](03-ws-vm-arithmetic-overflow.md) の仕様決定と連動）
- [ ] 修正・境界値テスト追加

## 問題点

- `src/optimizer/condition_opt.rs:240-270,316` 付近: `a < b` を `a - b < 0` に変換
- 同モジュールのドキュメント（`:27`）は「最適化なしの結果を変化させない（セマンティクス保持）」と明言しており、自ら定めた不変条件に違反
- `src/compiler_ws/expression.rs:290-313`（`comparison_spec`）も差分で比較を実装しており、WS 出力全般で同じ問題がある

### 再現: `a = 9223372036854775807; b = 0 - 1; if: a < b {1} else: {0}`

| 実行方法 | 結果 |
|---|---|
| nospace20 run（最適化なし） | 0（正しい） |
| nospace20 run `--opt condition-opt` / `--opt all` | 1（誤り） |
| WS コンパイル後を whitespace20 で実行（最適化有無とも） | `interpreter.rs:632` で panic |
| 同じ ws を wsc（多倍長）で実行 | 0 |

## 違反している原則

- 最適化パスの意味保存性

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | condition_opt の変換を `==` / `!=`（ゼロ比較）、または片方が定数 0 の場合に限定 | `condition_opt.rs`、`optimizer/tests.rs` |
| B | オーバーフローを仕様上「未定義 / エラー」と定義し、インタプリタを checked 演算にして「最適化で結果が変わる」状況を検出可能にする | 03 の案 C と同範囲 |
| C | `i64::MAX/MIN` 近傍の比較を `resources/tests/passes` に追加し、opt_all ターゲットとの差分を CI で検出 | テストのみ |

注: WS の比較命令は差分（`sub` + `jn`）でしか表現できないため、WS 出力側は wsc（多倍長）では正しく、`compiler_ws` 単体の問題というより VM 側（03）と仕様の問題。インタプリタ最適化パスは案 A で修正可能。
