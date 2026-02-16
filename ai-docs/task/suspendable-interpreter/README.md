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

### Phase 5: WASM API 実装 (Phase 1〜4 完了後)

nospace を直接ステップ実行する中断可能インタプリタの WASM API を実装する。

**前提条件:** Phase 1〜4 の完了（`InterpreterSession` / 再開機能が実装済み）

- [ ] `OwnedInterpreterSession` の実装（Scope 所有版セッション）
  - `InterpreterSession` が参照を持つため、WASM 境界をまたげない
  - Scope を所有し、ライフタイムフリーな構造を作成
- [ ] `WasmInterpreterSession` WASM API 実装
  - `new(source: &str, stdin: &str)` — セッション作成
  - `step(n: u32)` — n ステップ実行（`VmStepResult` を返却）
  - `get_stdout()` — 標準出力取得
  - `get_return_value()` — 終了時の戻り値取得
- [ ] デバッグ情報 API
  - `get_variables()` — 現在のスコープの変数一覧・値
  - `get_call_stack()` — 関数コールスタック
  - `get_position()` — 現在の実行位置（行・列）
- [ ] テスト・検証
  - Node.js スモークテスト（`tools/wasm-test/` にテストケース追加）
  - ブラウザでのマニュアル動作確認

## 関連タスク

- [wasm-build/](../wasm-build/) — WASM ビルド・基本 API (run / compile / Phase A は完了済み)
- Phase 5 は wasm-build タスクの Phase B に相当する機能を実装する
