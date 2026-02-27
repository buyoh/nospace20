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
| 1 | 代入文の値破棄最適化 | 共通 | 中 | 中 | **高** | [02-discard-assign-value.md](02-discard-assign-value.md) |
| 2 | 比較演算インライン化 | WS | 中 | 中 | 中 | [03-comparison-inline.md](03-comparison-inline.md) |
| 3 | 未使用変数削除 | 共通 | 高 | 小 | 低 | [04-dead-variable.md](04-dead-variable.md) |
| 4 | ピープホール最適化 | WS | 中 | 中 | 中 | [05-peephole.md](05-peephole.md) |

> **注**:
> - 短絡評価（`&&`/`||`）の問題はバグ修正であり、最適化タスクとは別に管理。→ [fix-short-circuit-evaluation.md](../fix-short-circuit-evaluation.md)
> - 末尾呼出し最適化は規模が大きいため独立タスクとして管理。→ [tail-call-optimization.md](../tail-call-optimization.md)

## Phase 1 との関係

```
Phase 1（実装済み）:
  constant-folding → condition-opt → geti-opt → dead-code

Phase 2（計画）:
  constant-folding → condition-opt → geti-opt →
  → comparison-inline → discard-assign-value → dead-code → peephole
```

- `comparison-inline` は `condition-opt` の後に実行（condition-opt 対象外の比較式を処理）
- `discard-assign-value` は中間表現レベルの最適化
- `peephole` は最終段の命令列最適化（他のパス完了後に実施）

## 各パス実装時の共通作業

各最適化パスの実装完了時に、以下のドキュメントも更新すること：

- `docs/optimize.md` — パス一覧への追加、パスの詳細説明セクション追加、実行順序の更新
