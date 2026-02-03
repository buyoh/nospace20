# 旧テストの移行 - Phase 3: else:if構文とI/Oテスト

## 日時
2026-02-04

## 目標
- `else:if:` 構文のパーサー対応
- legacy_009, legacy_010 の有効化
- 新しいI/Oテストケースの作成

## 背景

Phase 2 で7つのレガシーテスト（001-005, 007-008）を移行完了したが、以下のテストが未対応:

- **legacy_009, legacy_010**: `else:if:` 構文が未サポート（パーサー制約）
  - 既存の `control_flow/if_001` も同じ構文で失敗
  - これはパーサーの制約であり、機能未実装ではない

## タスク

### 1. パーサーの拡張（`else:if:` 構文サポート）

#### 調査事項
- [ ] 現在のelse文パーサーの実装を確認
- [ ] `else:if:` が失敗する原因を特定
- [ ] `} else:{` と `else:if:` の両方をサポートする方法を検討

#### 実装
- [ ] tree_parser で `else:if:` 構文をサポート
- [ ] 既存のテストが壊れないことを確認
- [ ] `control_flow/if_001` が通ることを確認

### 2. legacy_009, 010 の有効化

- [ ] パーサー修正後、legacy_009 が通ることを確認
- [ ] パーサー修正後、legacy_010 が通ることを確認
- [ ] 必要に応じて .check.json を調整

### 3. 新しいI/Oテストケースの作成

`resources/tests/passes/io/` ディレクトリに以下のテストを作成:

- [ ] `io_001.ns`: 基本的な入出力（__geti, __puti）
- [ ] `io_002.ns`: 文字入出力（__getc, __putc）
- [ ] `io_003.ns`: 複数回の入出力
- [ ] 各テストの .check.json ファイルを作成
- [ ] テストを実行して通ることを確認

## 完了条件

- [ ] `cargo test test_legacy_009 --test code_test` が成功
- [ ] `cargo test test_legacy_010 --test code_test` が成功
- [ ] `cargo test test_if_001 --test code_test` が成功（control_flow/if_001）
- [ ] io ディレクトリのテストが全て成功
- [ ] 既存のテストが壊れていない

## メモ

### 構文仕様
- if文: `if: cond { ... };` または `if: (cond) { ... };`
- else文: `else: { ... };`
- else if文: `else:if: cond { ... };` または `else:if: (cond) { ... };`
- 波括弧とキーワードの間のスペースは不要: `} else:{` も有効

## 参考
- Phase 2 完了報告: [ai-docs/done-task/legacy-migration-phase2-report.md](ai-docs/done-task/legacy-migration-phase2-report.md)
- 既存の失敗テスト: [resources/tests/fails/control_flow/if_001.ns](resources/tests/fails/control_flow/if_001.ns)
