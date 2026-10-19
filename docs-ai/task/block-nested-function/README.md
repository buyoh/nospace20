# ブロックスコープ内の関数宣言（禁止 → 許可）

ブロックスコープ（`{}`・`if`・`while`・`for`・`repeat` の本体、ブロックエイリアスの展開先）で宣言した関数の呼び出しが、別のグローバル関数に解決されて誤ったコードが生成される不具合への対応。まず意味解析エラーにして誤コード生成を止め（案 A）、次のバージョンでブロック内の関数宣言を正しく扱えるようにする（案 B）。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

元の指摘: [code-review-2026-10/02-block-nested-function-misresolution.md](../code-review-2026-10/02-block-nested-function-misresolution.md)
関連: [nested-block-function-and-static.md](../nested-block-function-and-static.md)（#1 は本タスクと同じ不具合、#3 は案 B で解消する。#2 の static 宣言は対象外）

## 背景・動機

`analyze_block_for_expression`（`src/semantic_analyzer/mod.rs:53-80`）は、コメントでは「ブロック式の中では新しい関数宣言は許可されない」としているが、実際には禁止していない。ブロック内の関数は一時的な空の Vec に登録されるため、グローバル関数のインデックスが 0 から振られる。解析後にその Vec は捨てられるので、呼び出しは本来のグローバル関数表の 0 番目（多くは `__main`）などを指す。エラーにならず、誤ったコードが生成される。

2026-10-19（コミット 9ad8d1f）時点の確認結果:

| 入力（要点） | 結果 |
|---|---|
| `func: __main(){ __putc(65); { func: g(){ __putc(66); return: 5; } g(); }; }` | `A` を無限に出力（`__main` が再帰している）。run・WS とも同じ |
| `func: h(){return:9;} func: __main(){ __puti({ func: g(){return:1;} g(); }); }` | `9` を出力（`h` が呼ばれる） |
| `for: { let: i(0); func: g(){ return: 7; } } { i < 1; } { i += 1; } { __puti(g()); };` | `semantic error: cannot use void expression as a value`。for-init は関数表を共有しているので `g` の解決先は正しいが、本体の型検査は init より前に作った戻り値型の一覧を使っており、`g` の戻り値型が見つからず Void 扱いになる |

for-init は関数表を共有しているので、`g` の呼び出し先は正しい。それでも「本体で `g` の戻り値を使う」と誤ったエラーになる。ブロック内の関数宣言は、どの形でも正しく動いていない。

## 設計・方針

| 段階 | 内容 | バージョン | ドキュメント |
|---|---|---|---|
| A | すべてのブロックスコープで関数宣言を意味解析エラーにする（テンプレートのインスタンス化・名前空間・ブロックエイリアス経由も含む）。spec に明記する | 3.0.0（現在の WIP）PATCH | [01-forbid-in-block.md](01-forbid-in-block.md) |
| B | 関数表を共有する `FunctionTable` を導入し、意味解析の全経路（式の変換を含む）に引き回す。A の禁止を外す。あわせて、ブロック越しに非 static 変数を参照できてしまう境界判定、名前空間内の関数でのネスト関数の panic、final・配列サイズの参照が境界を見ない問題を直す | `3.1.0 (WIP)`（3.0.0 リリース後）MINOR・PATCH | [02-allow-in-block.md](02-allow-in-block.md) |

- A と B は別コミットにし、VERSION.md の別バージョンに記載する。A を入れた時点で誤コード生成は止まる。B は A の検査を外す変更なので、A が前提
- B の Step 1（`FunctionTable` の導入と引き回し）は、A の検査が有効な間は挙動を変えない。リファクタとして単独のコミットで先に入れ、テストが通ることを確かめてから Step 2 で禁止を外す
- static 宣言をブロック内に書いたときの不具合（nested-block-function-and-static #2）は、A・B のどちらでも扱わない。B では、ブロックの static をそのブロック内の関数から参照する組み合わせだけを当面エラーにする

## タスクステップ・チェックリスト

1. [ ] A: ブロックスコープ内の関数宣言を禁止（[01-forbid-in-block.md](01-forbid-in-block.md)）
2. [ ] B: ブロックスコープ内の関数宣言を許可（[02-allow-in-block.md](02-allow-in-block.md)）
   1. [ ] Step 1: `FunctionTable` の導入と、式の変換を含む全経路への引き回し（挙動は変えない）
   2. [ ] Step 2: 禁止の解除・関数境界の判定修正・spec・テスト
