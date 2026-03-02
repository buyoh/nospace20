# インタプリタ中断・再開機能 (Suspendable Interpreter)

## 概要

nospace インタプリタの実行を特定のステップ数で中断し、後から再開できる機能を追加する。
`WhitespaceVM` と同様の**明示的スタックマシン**として新規実装し、`NospaceVM` として提供する。
既存の再帰インタプリタ (`interpret()` 系) はそのまま残し、用途に応じて選択可能にする。

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

### 設計方針

- **明示的スタックマシン**方式を採用（アプローチ A）
- `WhitespaceVM` (`src/whitespace/`) と同等のインターフェースを持つ `NospaceVM` を新設
- 既存の再帰インタプリタ (`src/interpreter/exec.rs`) は変更せず残す
- `src/interpreter/` モジュール内に `vm.rs` (または `vm/` サブモジュール) として追加

## ドキュメント

| ファイル | 内容 |
|---------|------|
| [approach-analysis.md](approach-analysis.md) | アプローチ比較・選定理由 |
| [detailed-design.md](detailed-design.md) | 詳細設計・型定義・コード変更箇所 |

## フェーズ計画

### Phase 1: 型と API の骨格

- [ ] `NospaceVM` 構造体の定義（`WhitespaceVM` 相当）
- [ ] `StepResult` enum の定義 (`Complete` / `Suspended` / `Error`)
- [ ] Builder パターンの実装 (`with_stdin`, `with_io`, `with_interactive_stdin` 等)
- [ ] `step(budget) -> StepResult` メソッドの骨格
- [ ] `Scope` を所有する設計（ライフタイムフリー、WASM 向け）
- [ ] `lib.rs` に `NospaceVM` / `StepResult` の re-export 追加

### Phase 2: 明示的スタックマシンの実装

- [ ] `Frame` enum の定義（文リスト / 式評価 / 関数呼び出し / while / for / if / block）
- [ ] `execute_step()` — 1ステップ実行（フレームスタックの先頭を処理）
- [ ] 式評価のフレーム化（再帰→ループ+スタック変換）
- [ ] 文実行のフレーム化
- [ ] 関数呼び出し・復帰のフレーム化
- [ ] ループ (while, for) のフレーム化
- [ ] if/block 式のフレーム化
- [ ] グローバル初期化のフレーム化

### Phase 3: テスト・統合

- [ ] 既存テストケースが `NospaceVM` でも全て通ることの確認
- [ ] `step(1)` での1式ずつ実行→再開のユニットテスト
- [ ] `max_expression_count` 相当の動作確認（Suspended で止まり、再度 step で継続可能）
- [ ] 再帰版インタプリタとの結果一致テスト

### Phase 4: WASM API 実装

- [ ] 再帰インタプリタの WASM API 削除
  - `api.rs` の `run()` 関数を削除（`interpret_with_env` を使用しているため）
  - 関連する `RunResultOk` / `JsRunResult` 型の整理
- [ ] `WasmNospaceVM` WASM ラッパーの実装（`src/wasm_api/nospace_vm.rs` 新規作成）
  - `new(source, stdin, interactive?, opt_passes?, ignore_debug?)` — VM 構築
  - `step(budget)` — N ステップ実行
  - `flush_stdout()` — 標準出力取得
  - `is_complete()` — 完了判定
  - `total_steps()` — 総実行ステップ数
  - `get_return_value()` — main の戻り値
  - `get_traced()` — トレース情報
  - `provide_stdin(data)` / `close_stdin()` — interactive stdin
- [ ] `src/wasm_api/mod.rs` に `mod nospace_vm;` 追加
- [ ] デバッグ情報 API（将来拡張）
  - `get_call_stack()` — 関数コールスタック
  - `get_position()` — 現在の実行位置
- [ ] テスト・検証
  - Node.js スモークテスト（`tools/wasm-test/` にテストケース追加）

## 既存インタプリタとの共存

| 機能 | 再帰インタプリタ (`interpret()`) | スタックマシン (`NospaceVM`) |
|------|------|------|
| 用途 | CLI ワンショット実行、テスト | WASM ステップ実行、中断・再開 |
| WASM 利用 | **不可**（`run()` API 削除） | 可能（`WasmNospaceVM`） |
| 中断・再開 | 不可 | 可能 |
| 実装の複雑さ | シンプル | 複雑（フレーム定義） |
| パフォーマンス | 高速（Rust ネイティブスタック） | やや遅い（ヒープ上のスタック） |
| 変更方針 | 変更せず維持 | 新規実装 |

## 関連タスク

- [wasm-build/](../wasm-build/) — WASM ビルド・基本 API (run / compile / Phase A は完了済み)
- Phase 4 は wasm-build タスクの Phase B に相当する機能を実装する
- Phase 4 で既存の `run()` API を削除し、`WasmNospaceVM` で置き換える
- WASM API の詳細設計は [detailed-design.md](detailed-design.md) の「WASM API 設計」セクション参照
