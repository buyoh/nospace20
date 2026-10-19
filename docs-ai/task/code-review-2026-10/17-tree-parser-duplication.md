# tree_parser の重複コードと巨大モジュール

- 重要度: **low**
- 状態: 設計完了（未実装）。設計: [tree-parser-dedup](../tree-parser-dedup/README.md)

## 進捗

- [ ] 共通カーソルの抽出
- [ ] statement/mod.rs の分割

## 問題点

- `parse_type_spec` が `src/tree_parser/statement/mod.rs:559-634` と `src/tree_parser/expression/mod.rs:658-731` に実質同一のコードとして存在（差分は閉じ括弧とコメントのみ）
- 引数リストのループが `expression/mod.rs:154-195` と `238-280` に重複（[01](01-parser-infinite-loop.md) のバグが 2 箇所にある原因）
- `add_parse_error` / `add_end_error` / 現在位置取得が 2 つの Builder に重複し、挙動も異なる（`current_pos()` は EOF で 0、`current_pos_or(start)` は start）
- `statement/mod.rs` は 2010 行あり、既存の分割タスク（code-design-review/02-module-splitting）の対象にも入っていない
- `expression/mod.rs:271` の `args.push((*e).clone())` は Box を不要に深くコピー

## 違反している原則

- DRY、SRP

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | 共通の `ParserCursor`（イテレータ・エラー Vec・位置取得）を切り出し両 Builder に持たせる。`parse_type_spec` もそこへ移動 | `tree_parser` 内部のみ（公開 API 不変） |
| B | `statement/mod.rs` を宣言系（let/constexpr/alias/struct/func）と制御構文系（for/repeat/while/if）に分割 | `tree_parser/statement` のみ。02-module-splitting に統合しても良い |
