# コンパイル時最適化

意味解析後の中間表現 (`Scope` / `ExecExpression` / `ExecStatement`) に対して最適化パスを適用し、生成コードの効率を向上させる。

## 背景

現在のパイプライン:

```
Token Parser → Tree Parser → Semantic Analyzer → Scope → Interpreter / Compiler WS
```

最適化パスを意味解析の後に挿入する:

```
Token Parser → Tree Parser → Semantic Analyzer → Scope → [Optimizer] → Interpreter / Compiler WS
```

## 設計ドキュメント

| ドキュメント | 内容 |
|---|---|
| [01-pass-framework.md](01-pass-framework.md) | 最適化パスフレームワーク設計（モジュール構成、パスの実行制御、ExecExpression 拡張） |
| [02-pass-condition-opt.md](02-pass-condition-opt.md) | if/while 条件式最適化（JumpIfZero/JumpIfNegative の直接利用） |
| [03-pass-geti-opt.md](03-pass-geti-opt.md) | `__geti` / `__getc` 入力最適化（一時領域経由の排除） |
| [04-pass-dead-code.md](04-pass-dead-code.md) | 未使用関数・変数の削除 |
| [05-pass-constant-folding.md](05-pass-constant-folding.md) | 定数畳み込み |

## 最適化パス一覧と優先度

| # | パス名 | 対象バックエンド | 難易度 | 効果 | 優先度 |
|---|---|---|---|---|---|
| 1 | 定数畳み込み | 共通 | 低 | 中 | **高** |
| 2 | if/while 条件式最適化 | Whitespace | 中 | **大** | **高** |
| 3 | `__geti`/`__getc` 最適化 | Whitespace | 低 | 中 | 中 |
| 4 | 未使用関数削除 | 共通 | 中 | 中 | 中 |
| 5 | 未使用変数削除 | 共通 | 高 | 小 | 低 |

## 評価方法

既存の Whitespace VM プロファイラ (`examples/ws_profiler.rs`) を使用して最適化の効果を測定する。

- **生成命令数**: コンパイル後の Whitespace 命令数
- **実行ステップ数**: VM での実行ステップ数
- **メモリアクセス範囲**: ヒープ使用量

比較レポートは `tools/profile-report.py` で HTML 生成可能。

## 実装順序

1. **フレームワーク構築** — `src/optimizer/` モジュール作成、パイプライン統合 ✅ 完了
2. **定数畳み込み** — 最もシンプルで汎用的
3. **条件式最適化** — 効果が最大。ExecExpression の拡張を伴う
4. **`__geti`/`__getc` 最適化** — パターンマッチで実装可能
5. **未使用関数削除** — 到達可能性解析が必要

## フレームワーク実装状況

### 完了済み

- `src/optimizer/mod.rs` — パス管理・実行エントリポイント
- `src/optimizer/noop_test_pass.rs` — 動作検証用ダミーパス（マジックナンバー `0xDEAD` のグローバル変数を追加）
- `src/optimizer/tests.rs` — ユニットテスト 5 件
- `src/lib.rs` — `optimize()` 公開 API
- `src/compile_property.rs` — `optimization_level` フィールド追加
- CLI `--opt` オプション追加
- アーキテクチャドキュメント更新
