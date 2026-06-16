# whitespace-self テストターゲット追加

## 概要

`whitespace-self` テストターゲットを追加。nospace コードを Whitespace にコンパイルし、独自 WhitespaceVM で実行して動作確認するモード。

## 変更内容

- `build.rs`: `success`/`success_io` テストタイプに `_ws_self` テスト関数を自動生成
- `tests/code_test.rs`: `test_whitespace_self_base` と `test_whitespace_self_io_base` を追加
- `resources/tests/test-manifest.yaml`: `exclude_targets` に `whitespace-self` を追加可能に

## テスト結果

- 246 passed, 15 failed (all `_ws_self`), 113 ignored
- 既存テストへの影響なし
- 15件の失敗は既存の whitespace コンパイラ・VM の問題（調査ドキュメント: `docs-ai/task/whitespace-self-test-failures.md`）

## 完了日

2026-02-17
