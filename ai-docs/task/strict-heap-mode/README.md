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
| 5 | 変数の初期値を「未定義」に仕様変更 | [undefined-variable-init.md](undefined-variable-init.md) | - |
| 6 | ランダム初期化モードの追加 | [randomize-uninit-mode.md](randomize-uninit-mode.md) | 5 |

## 設計方針

- `--std-ext` ではなく専用の `--strict-heap` フラグとする（`wsc` と同様に独立したオプション）
- テストでは `test-manifest.yaml` に strict-heap テストバリアントを生成する仕組みを追加
- 既存テストを壊さないよう、strict-heap はデフォルト無効
- 変数の初期値は仕様上「未定義」とし、インタプリタ・VM の両方でランダム初期化モードによる検出手段を提供

## 更新履歴

- 2026-02-18: 初版作成
- 2026-02-18: Phase 1〜3 実装完了
  - Phase 1: `WhitespaceVM` に `strict_heap` フィールド、`with_strict_heap()` builder、`heap_retrieve` の strict モード対応を追加
  - Phase 2: `whitespace20` CLI に `--strict-heap` オプションを追加
  - Phase 3: `tests/code_test.rs` にヘルパー関数（`test_whitespace_self_base_strict`、`test_whitespace_self_io_base_strict`）を追加、`build.rs` に `whitespace-self-strict` ターゲット生成ロジックを追加
  - 失敗した 6 件のテストを `exclude_targets: [whitespace-self-strict]` で除外（[調査ドキュメント](strict-heap-test-failures.md)）
- 2026-02-18: Phase 5〜6 実装完了（Phase 4 スキップ）
  - Phase 5: `spec.md` の変数初期値を「未定義」に仕様変更
  - Phase 6: `EnvironmentConfig` に `randomize_uninit` フラグを追加、インタプリタ全変数初期化箇所をランダム値対応に変更
  - Phase 6: `WhitespaceVM` に `randomize_heap` フラグと `with_randomize_heap()` builder を追加
  - Phase 6: `whitespace20` CLI に `--randomize-heap` オプションを追加
  - Phase 6: `interpreter-randomize` / `whitespace-self-randomize` テストターゲットをテスト基盤に追加
  - randomize テストで失敗した 11 件（未初期化変数依存）は `exclude_targets` に追加せず TODO として管理（[調査ドキュメント](strict-heap-test-failures.md)）
