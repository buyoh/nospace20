# シンボルテーブル実装 — 詳細設計

親ドキュメント: [symbol-table-design.md](../symbol-table-design.md)

本ディレクトリはステップ5・6の詳細な実装設計を格納する。

## ドキュメント一覧

| ファイル | 内容 | 対応ステップ |
|---------|------|------------|
| [step5-static-storage-indexing.md](step5-static-storage-indexing.md) | function_static_storage のインデックスキー化 | ステップ5 |
| [step6-symbol-table.md](step6-symbol-table.md) | SymbolTable 構造体の導入と文字列情報の分離 | ステップ6 |

## 依存関係

```
ステップ5 → ステップ6
```

ステップ5 は独立して実装可能。ステップ6 はステップ5 の完了が前提。
