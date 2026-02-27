# Step 6: 仕様書・ドキュメントの更新

## 概要

言語仕様書やアーキテクチャ文書を更新し、while が式から文に変更されたことを反映する。

## 変更内容

### 6-1. docs/spec.md

#### セクション「while 式」→「while 文」

変更前:
```markdown
### while 式
...
- while は式である。
- while 式の型は void である。値として使用することはできない。
```

変更後:
```markdown
### while 文
...
- while は文である。式として使用することはできない。
```

#### 型システムのテーブルから while を削除

変更前:
```markdown
| while | 常に void |
```

変更後: この行を削除。while は文であり型を持たない。

### 6-2. docs/grammar.bnf

現状の BNF は既に `while_stmt` として定義されており、`expr_val` には含まれていない。
変更不要。

### 6-3. ai-docs/architecture/modules.md

`Expression` enum の説明から `While` を削除し、`Statement` enum の説明に `While` を追加。

### 6-4. docs/tutorial.md

while の使用例が式として説明されている箇所がないか確認し、必要に応じて更新。

## 確認ポイント

- 仕様書の while に関する記述が一貫していること
- 「式」「文」の用語が正確に使われていること
