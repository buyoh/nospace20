---
name: add-test-spec
description: /resources/tests/ 以下にテストケースを追加するときに使う
---

言語 `nospace` は、コード中の任意の箇所のスペース、改行、タブを許容する、遊びを目的としたプログラミング言語である。

- 言語 `nospace` の仕様は `spec.md` に記載
- /spec.md に基づき 、/resources/tests/ 以下にテストケースを追加
- 追加したテストケースは `/resources/tests/test-manifest.yaml` へ登録
- テストは `build.rs` により自動生成される

## テストケースの追加手順

1. テストファイルを作成
   - `resources/tests/passes/` (成功するテスト) または `resources/tests/fails/` (失敗するテスト) に配置
   - `.ns` ファイル: nospace ソースコード
   - `.check.json` ファイル: 期待する結果

2. `test-manifest.yaml` に登録
   - `name`: テスト関数名
   - `type`: `success`, `success_io`, `syntax_error`, `compile_error` など
   - `path`: テストファイルのパス（相対パス）
   - `comment`: (オプション) テストの説明
   - `targets`: (オプション) `[interpreter, whitespace]` など

## success_io テストで複数ケースを使う

1つのテストに複数の入出力パターンを定義できます:

```json
{
  "type": "success_io",
  "cases": [
    {
      "name": "positive",
      "stdin": "42\n",
      "stdout": "42"
    },
    {
      "name": "zero",
      "stdin": "0\n",
      "stdout": "0"
    }
  ]
}
```

後方互換性のため、`cases` を使わない従来の形式も引き続き利用可能です。

詳細は `/resources/tests/README.md` を参照してください。