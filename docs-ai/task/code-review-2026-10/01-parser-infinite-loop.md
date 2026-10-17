# 構文解析: 引数リスト中の不正トークンで無限ループ・メモリ無制限消費

- 重要度: **high**
- 状態: 未着手

## 進捗

- [ ] 修正方針の決定
- [ ] 修正・fails/syntax テスト追加

## 問題点

関数呼び出し・struct リテラルの引数ループで、factor が想定外のトークンを受け取ったとき `add_parse_error` のみ行いトークンを消費しない。引数ループは同じ位置から `parse_to_expression_tree_root()` を再度呼ぶため、イテレータが進まずエラーと引数が無限に push される。

- `src/tree_parser/expression/mod.rs:160-195`（関数呼び出し引数ループ）
- `src/tree_parser/expression/mod.rs:244-280`（struct リテラル引数ループ）
- `src/tree_parser/expression/mod.rs:341-346`（factor のフォールバック分岐）

### 再現（確認済み）

```
func: __main(){ f(;          → timeout (rc=124)、1.5秒で RSS 約 1.2GB
f(]);  f(}  f(:  let: x(struct: S(;));  f([2,3])   → いずれも同様
```

WASM の `parse()` / `compile()` / `WasmNospaceVM::new` は同期処理のため、エディタで `f(` と入力途中のコードを渡すだけでブラウザタブがフリーズ・OOM になる。入力サイズ制限では防げない。

## 違反している原則

- フェイルセーフ設計: エラー回復処理が入力の前進を保証していない
- DRY: 同じループが 2 箇所にあり、同じバグも 2 箇所にある（[17-tree-parser-duplication.md](17-tree-parser-duplication.md)）

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | factor のフォールバック分岐で `;` `}` `)` `]` 以外の不正トークンを消費する | `tree_parser/expression/mod.rs`。他構文のエラー回復にも効くため、`resources/tests/fails/syntax` のエラー件数・メッセージがずれる可能性 |
| B | 引数ループで root 呼び出し前後のトークン位置を比較し、進んでいなければ 1 トークン消費するか `Invalid` で抜ける | `tree_parser/expression/mod.rs` のみ |
| C | 2 つの引数ループを `parse_comma_separated_args()` に統合し、そこで進捗を保証する | `tree_parser/expression/mod.rs` のみ。17 と同時に解消 |

推奨: C（内部で B の進捗チェックを行う）。
