# コンパイル最適化 Phase 2

Phase 1 で実装済みの最適化パス（`constant-folding`, `condition-opt`, `geti-opt`, `dead-code`）に続く追加最適化の設計・計画。

## 背景

Phase 1 では意味解析後の `Scope` に対する最適化パスを導入した。Phase 2 では、残存する非効率パターンへの対処と、Whitespace 命令レベルでのポスト最適化を検討する。

### プロファイルデータからの知見

fibonacci（10653 ステップ）・qsort（5354 ステップ）のプロファイルから：

| 命令種別 | fibonacci | qsort | 観察 |
|---|---|---|---|
| push | 27.3% | 31.5% | 最多。定数プッシュ・アドレス計算が支配的 |
| retrieve | 12.3% | 24.8% | メモリ読出し。代入後の再読出しが多い |
| add | 6.7% | 14.4% | アドレス計算（ローカル変数アクセス）が多い |
| call/return | 12.4% | 3.2% | fibonacci は再帰が多いため顕著 |
| swap | 9.8% | 4.8% | スタック操作のオーバーヘッド |
| label | 11.0% | 4.6% | NOP だが実行カウントされる |

## 最適化候補一覧

| # | パス名 | 対象 | 難易度 | 効果 | 優先度 | ドキュメント |
|---|---|---|---|---|---|---|
| 1 | 短絡評価インライン化 | WS | 中 | 大 | **高** | [01-short-circuit-inline.md](01-short-circuit-inline.md) |
| 2 | 代入文の値破棄最適化 | 共通 | 中 | 中 | **高** | [02-discard-assign-value.md](02-discard-assign-value.md) |
| 3 | 比較演算インライン化 | WS | 中 | 中 | 中 | [03-comparison-inline.md](03-comparison-inline.md) |
| 4 | 未使用変数削除 | 共通 | 高 | 小 | 低 | [04-dead-variable.md](04-dead-variable.md) |
| 5 | ピープホール最適化 | WS | 中 | 中 | 中 | [05-peephole.md](05-peephole.md) |
| 6 | 末尾呼出し最適化 | WS | 高 | 大 | 低 | [06-tail-call.md](06-tail-call.md) |

## Phase 1 との関係

```
Phase 1（実装済み）:
  constant-folding → condition-opt → geti-opt → dead-code

Phase 2（計画）:
  constant-folding → short-circuit-inline → condition-opt → geti-opt →
  → comparison-inline → discard-assign-value → dead-code → peephole
```

- `short-circuit-inline` は `condition-opt` の前に実行（条件式パターンに影響する可能性）
- `comparison-inline` は `condition-opt` の後に実行（condition-opt 対象外の比較式を処理）
- `discard-assign-value` は中間表現レベルの最適化
- `peephole` は最終段の命令列最適化（他のパス完了後に実施）
- `tail-call` は独立したパス（将来的な実装候補）
