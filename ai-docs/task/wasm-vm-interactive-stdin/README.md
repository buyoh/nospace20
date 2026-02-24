# WASM WhitespaceVM Interactive Stdin

## 概要

`WasmWhitespaceVM` に interactive な入出力のための stdin 一時停止機能を追加する。
現在の実装では stdin をコンストラクタで全て事前に提供する必要があるが、ブラウザ環境ではユーザーの入力をリアルタイムに受け付けたい。

## 背景・問題

### 現状

- `WasmWhitespaceVM::new()` / `from_whitespace()` で stdin を `Cursor<Vec<u8>>` として事前構築
- `InputChar` / `InputNumber` 命令実行時、stdin バッファが空だと:
  - `read_char`: EOF (0) を返す
  - `read_number`: パースエラーで `RuntimeError::IoError` になる
- JS 側から後から入力を追加する手段がない

### 要件

1. `InputChar` / `InputNumber` 命令で stdin が不足した場合、VM を一時停止する
2. JS 側から追加の stdin データを提供できる
3. 提供後に `step()` を再呼び出しすると、入力命令をリトライして実行継続する
4. 一時停止の種別（文字入力待ち / 数値入力待ち）を JS 側で区別できる
5. 既存の非 interactive な使い方（stdin 事前全提供）は動作を変えない

## ドキュメント

| ファイル | 内容 |
|---------|------|
| [design.md](design.md) | 詳細設計（型定義・変更箇所・実装方針） |

## フェーズ計画

### Phase 1: WhitespaceVM の StepResult 拡張

WhitespaceVM 内部（`src/whitespace/interpreter.rs`）に入力待ちの概念を追加する。

- [ ] `StepResult::WaitingForInput` バリアントの追加
- [ ] `ExecuteResult::WaitingForInput` バリアントの追加
- [ ] stdin を追記可能なバッファ（`InteractiveStdin`）の実装
- [ ] `InputChar` / `InputNumber` 命令の分岐: バッファ不足時に WaitingForInput を返す
- [ ] `WhitespaceVM::with_interactive_stdin()` ビルダーメソッドの追加
- [ ] ユニットテスト

### Phase 2: WASM API の拡張 (`src/wasm_api.rs`)

JS 側から利用可能な API を追加する。

- [ ] `VmStepResult` TypeScript 型に `"waiting_for_input"` ステータスと `inputType` フィールド追加
- [ ] `WasmWhitespaceVM::provide_stdin(data: &str)` メソッドの追加
- [ ] コンストラクタで interactive stdin を利用するモードの追加
- [ ] `VmStepResult` のシリアライズ更新

### Phase 3: テスト・統合

- [ ] WASM ビルド確認
- [ ] Node.js テスト（`tools/wasm-test/` にテストケース追加検討）
- [ ] 既存テストの回帰確認

## 関連

- [suspendable-interpreter/](../suspendable-interpreter/) — nospace インタプリタの中断・再開機能（別タスク）
- `src/whitespace/interpreter.rs` — WhitespaceVM 実装
- `src/wasm_api.rs` — WASM 公開 API
