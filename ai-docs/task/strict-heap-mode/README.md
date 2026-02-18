# Whitespace VM strict-heap モード

## 概要

外部インタプリタ `wsc` はデフォルトで未アクセスのヒープ読み込みをエラーとする。現在テストでは `--unchecked-heap` フラグでこのチェックを無効化しているが、組み込み WhitespaceVM にも同等の機能を追加し、テストで有効化できるようにする。

## 背景

- `wsc` はデフォルトでヒープの未初期化アドレスへの `retrieve` をエラーとして扱う
- 現在の組み込み WhitespaceVM では未初期化アドレスは 0 を返す（`unwrap_or(&0)`）
- テスト実行時、`wsc` に `--unchecked-heap` を渡してエラーを回避している（[tests/common/mod.rs](../../../tests/common/mod.rs#L80)）
- nospace コンパイラが正しく動作していれば、ユーザ変数の初期化忘れを除き、未初期化ヒープアクセスは発生しないはず

## 目的

1. 組み込み WhitespaceVM に strict-heap モードを追加
2. strict-heap 有効時にテストを実行できる仕組みを整備
3. コンパイラのバグ（不要な未初期化読み出し）を検出可能にする

## サブタスク

| # | タスク | ドキュメント | 依存 |
|---|--------|-------------|------|
| 1 | WhitespaceVM に strict-heap モードを追加 | [vm-changes.md](vm-changes.md) | - |
| 2 | CLI に `--strict-heap` オプションを追加 | [cli-changes.md](cli-changes.md) | 1 |
| 3 | テスト基盤に strict-heap テスト実行を追加 | [test-infrastructure.md](test-infrastructure.md) | 1 |
| 4 | wsc テストの `--unchecked-heap` 除去を検討 | [wsc-test-changes.md](wsc-test-changes.md) | 3 |

## 設計方針

- `--std-ext` ではなく専用の `--strict-heap` フラグとする（`wsc` と同様に独立したオプション）
- テストでは `test-manifest.yaml` に strict-heap テストバリアントを生成する仕組みを追加
- 既存テストを壊さないよう、strict-heap はデフォルト無効

## 更新履歴

- 2026-02-18: 初版作成
