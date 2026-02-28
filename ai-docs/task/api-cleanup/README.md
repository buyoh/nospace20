# API クリーンアップ

日付: 2026-03-01

## 概要

コード設計レビューで挙がった改善点のうち、公開 API・エラー型・モジュール境界に関する3つのタスクをまとめたもの。

## タスク一覧

| ファイル | 内容 | 状態 |
|----------|------|------|
| [03-deprecated-migration.md](03-deprecated-migration.md) | deprecated 関数の利用箇所を新 API に移行し、deprecated 関数を削除 | 未着手 |
| [04-ws-shared-types.md](04-ws-shared-types.md) | Whitespace 共有型を compiler_ws から独立モジュールに移動し依存方向を改善 | 未着手 |
| [05-error-type-unification.md](05-error-type-unification.md) | エラー型を `src/base/error` モジュールに集約し統一 | 未着手 |

## 依存関係

```
05-error-type-unification (base/error モジュール作成)
    ↓ SourceLocation を base 内で共有
04-ws-shared-types (共有型の移動)
    ↓ compiler_ws の公開 API が変わるため
03-deprecated-migration (deprecated 削除)
```

推奨実行順序: **05 → 04 → 03**（ただし各タスクは独立して実行も可能）
