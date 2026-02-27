# while を式から文へ変更

## 概要

`while` を式（Expression）から文（Statement）に変更する。
現在 `while` は `if` やブロックスコープ式と同様に式として扱われているが、
`while` は常に void 型であり値として使用できないため、`return` / `break` / `continue` と同じ文として扱うべきである。

## 動機

- `while` は常に void 型を返し、値として使用できない（代入・引数・条件式での使用はコンパイルエラー）
- 式として扱う必要性がない
- `return` 等と同じ文にすることで、言語仕様とコード構造がシンプルになる
- コンパイラのスタック管理が簡素化される（式として 0 をプッシュする必要がなくなる）

## 設計方針

### 構文

**変更前（式）:**
```
while: cond { body };   # 式文として `;` が必要 #
```

**変更後（文）:**
```
while: cond { body };   # 文の構文として `;` を維持 #
```

構文上は同じ `while: cond { body };` のまま。`;` は while 文の構文の一部として維持する（後方互換性）。

ただし、以下のような式としての使用は構文エラーとなる:
```
# 以下は変更後にパースエラー #
x = while: cond { body };     # 不可: 式ではない #
f(while: cond { body });      # 不可: 式ではない #
```

### `if` との差異

`if` は式のまま維持する。理由:
- `if` は else分岐と組み合わせることで int 型の値を返せる
- `x = if: cond { a } else: { b };` のような値としての使用が有用

`while` は常に void であり、値としての使用場面がないため文に変更する。

## ステップ

### ドキュメント

- [step1-tree-parser.md](step1-tree-parser.md) - 構文解析: Expression::While → Statement::While
- [step2-semantic-analyzer.md](step2-semantic-analyzer.md) - 意味解析: ExecExpression::While → ExecStatement::While
- [step3-interpreter.md](step3-interpreter.md) - インタプリタの更新
- [step4-compiler-ws.md](step4-compiler-ws.md) - Whitespace コンパイラの更新
- [step5-optimizer.md](step5-optimizer.md) - 最適化パスの更新
- [step6-spec-docs.md](step6-spec-docs.md) - 仕様書・ドキュメントの更新
- [step7-tests.md](step7-tests.md) - テストの更新

## 進捗

- [x] Step 1: tree_parser 変更完了
- [x] Step 2: semantic_analyzer 変更完了
- [x] Step 3: interpreter 変更完了
- [x] Step 4: compiler_ws 変更完了
- [x] Step 5: optimizer 変更完了 (condition_opt, dead_code, geti_opt, constant_folding, tests)
- [x] Step 6: spec/docs 更新完了 (docs/spec.md, ai-docs/architecture/modules.md)
- [x] Step 7: テスト更新完了 (while_as_expression_001 追加, void_while_assign_001 を syntax_error に変更)
- [x] 全テスト通過確認 (936 passed; 0 failed)

### 依存関係

```
Step 1 (tree_parser)
  ↓
Step 2 (semantic_analyzer)
  ↓
Step 3 (interpreter) ← 独立
Step 4 (compiler_ws) ← 独立
Step 5 (optimizer)   ← 独立
  ↓
Step 6 (spec/docs)   ← 独立
Step 7 (tests)       ← 全ステップ完了後
```

Step 1 → Step 2 は順序依存。Step 3/4/5 は Step 2 完了後に並行作業可能。

## 影響範囲サマリ

| モジュール | ファイル | 変更内容 | 規模 |
|---|---|---|---|
| tree_parser (expression) | `src/tree_parser/expression/mod.rs` | `Expression::While` 削除、factor パース削除 | 小 |
| tree_parser (statement) | `src/tree_parser/statement/mod.rs` | `Statement::While` 追加、while パース追加 | 小 |
| semantic_analyzer (types) | `src/semantic_analyzer/types.rs` | `ExecExpression::While` → `ExecStatement::While` | 小 |
| semantic_analyzer (main) | `src/semantic_analyzer/mod.rs` | while の変換を式→文レベルに移動 | 中 |
| interpreter | `src/interpreter/exec.rs` | while 解釈を式→文レベルに移動 | 中 |
| compiler_ws (expression) | `src/compiler_ws/expression.rs` | `generate_while_expression` 削除 | 小 |
| compiler_ws (statement) | `src/compiler_ws/statement.rs` | `generate_while_statement` 追加、変数カウント更新 | 中 |
| optimizer (condition_opt) | `src/optimizer/condition_opt.rs` | While 処理を式→文レベルに移動 | 中 |
| optimizer (dead_code) | `src/optimizer/dead_code.rs` | While 処理を式→文レベルに移動 | 小 |
| optimizer (geti_opt) | `src/optimizer/geti_opt.rs` | While 処理を式→文レベルに移動 | 小 |
| optimizer (tests) | `src/optimizer/tests.rs` | ConditionMode 置換ヘルパー更新 | 小 |
| 仕様書 | `docs/spec.md` | \"while 式\" → \"while 文\" | 小 |
| BNF | `docs/grammar.bnf` | 変更不要（既に while_stmt） | なし |
| アーキテクチャ文書 | `ai-docs/architecture/modules.md` | enum 定義の更新 | 小 |
