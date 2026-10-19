# 位置情報の単位の誤記とドキュメントコメント不足

- 重要度: **low**
- 状態: 計画 docs-ai/task/cli-errors-and-doccomments/README.md

## 進捗

- [ ] SourceLocation / code_pointer の単位明記
- [ ] EOF 時の end 位置修正
- [ ] 欠落ドキュメントコメント追加

## 問題点

1. **単位の誤記**: `src/base/location.rs:8-12` は「バイト単位のインデックス」と記述しているが、実際は `chars().enumerate()` による文字インデックス（`src/token_parser/mod.rs:619-623`）。`logger::TextCode` も文字前提。ドキュメントを信じて `&src[start..end]` で切り出すとマルチバイト文字で panic する。`TokenInfo.code_pointer`（`token_parser/mod.rs:76-78`）にはドキュメントコメントがない。
2. **EOF で start > end**: `src/tree_parser/expression/mod.rs:127-133` で EOF 時 `end=0` になり、不正な範囲が作られる。
3. **構造体・enum の概要ドキュメントコメント欠落**: `Keyword`、`Token`、`TokenInfo`、`Statement`（statement/mod.rs:41）、`Expression` / `Operator1` / `Operator2`（expression/mod.rs:17,40,65）、`TextCode`（logger/mod.rs:1）、`FunctionIndex`（scope.rs:13、タプルの意味が行末コメントのみ）、`ImportDecl` / `ImportBundle`。
4. **誤ったコメント**: `token_parser/mod.rs:345`「空白文字も含む」— 空白は事前除去済みで文字列内にも残らない（spec 251 行目）。

## 違反している原則

- AGENTS.md「構造体の概要は必ずドキュメントコメント」「時間・長さに関する値は単位を明示」

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `code_pointer` を `char_index` 等にリネーム、または doc に「文字インデックス（バイトではない）」と明記。`SourceLocation` の doc 修正 | リネームなら token_parser / tree_parser / テストに広く波及。doc 修正のみなら局所的 |
| B | EOF 時の end を start（または最後のトークン位置）に揃える | `tree_parser/expression/mod.rs` |
| C | 欠落しているドキュメントコメントを追加、誤ったコメントを修正 | 各ファイル（挙動変更なし） |
