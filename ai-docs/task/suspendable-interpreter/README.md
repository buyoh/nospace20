# インタプリタ中断・再開機能 (Suspendable Interpreter)

## 概要

インタプリタの実行を特定のステップ数で中断し、後から再開できる機能を追加する。
ブラウザ環境で長時間実行プログラムによるUIフリーズを防ぐためのコア機能。

## 背景

### 現状の問題

- `interpret()` を呼ぶと完了まで制御が戻らない
- `max_expression_count` による制限は `panic!` で異常終了するのみで、再開不可
- WASM 環境ではシングルスレッドでメインスレッドをブロックする

### 要件

1. N ステップ実行したら制御を呼び出し元に返す
2. 返された後、任意のタイミングで実行を再開できる
3. 完了 / 中断中 / エラー の状態を区別できる
4. native CLI モードでの既存動作に影響を与えない

## ドキュメント

| ファイル | 内容 |
|---------|------|
| [approach-analysis.md](approach-analysis.md) | アプローチ比較・選定理由 |
| [detailed-design.md](detailed-design.md) | 詳細設計・型定義・コード変更箇所 |

## フェーズ計画

### Phase 1: 型と API の整備

- [ ] `InterpreterSession` 構造体の定義
- [ ] `StepResult` enum の定義 (`Complete` / `Suspended` / `Error`)
- [ ] `lib.rs` に `interpret_start` / `interpret_resume` 公開 API 追加
- [ ] 既存の `interpret` / `interpret_func` が内部でセッションを使うようリファクタ

### Phase 2: 再帰インタプリタへの Yield 導入

- [ ] `Flow` / `ExpressionFlow` に `Yield` バリアント追加
- [ ] `increment_expression_count` を `check_step_budget` に変更（panic → Yield 返却）
- [ ] `Yield` の伝播処理を全 `interpret_*` メソッドに追加
- [ ] `LocalEnvironment` の状態を `InterpreterSession` に保存できるようにする

### Phase 3: 状態の保存と復元

- [ ] コールスタックの明示的な保存構造 (`ContinuationFrame`) の設計・実装
- [ ] `interpret_expression` / `interpret_statement` の継続ポイント定義
- [ ] while ループの反復状態の保存・復元
- [ ] 関数呼び出しの引数評価途中の保存・復元

### Phase 4: テスト・統合

- [ ] 1ステップ実行→再開のユニットテスト
- [ ] 既存テストケースが全て通ることの確認
- [ ] WASM API (`wasm-build` タスク) との統合

## 関連タスク

- [wasm-build/](../wasm-build/) — WASM API で `run` 関数が内部的に本機能を使用する予定
