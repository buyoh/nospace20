# 再帰深度の上限がなく、深いネスト入力・深い再帰でスタックオーバーフロー abort

- 重要度: **high**
- 状態: 完了 (2026-10-19) → [runtime-robustness-2026-10](../runtime-robustness-2026-10/README.md)

## 進捗

- [x] パーサにネスト深度上限を導入
- [x] インタプリタの呼び出し深度対策
- [x] fails テスト追加

## 問題点

### 1. コンパイラフロントエンド（入力のネスト）

- `src/tree_parser/expression/mod.rs`（factor ↔ root の相互再帰。328-337 括弧、427 単項演算子）
- `src/tree_parser/statement/mod.rs`（ブロックのネスト）
- `semantic_analyzer` の式変換も無制限に再帰

再現: `return: ((((...1...))))` 括弧深さ約 2000（debug）/ 20000（release）で `fatal runtime error: stack overflow, aborting`。単項 `-` 200000 個、`{` 100000 段でも同様。WASM はスタックが小さく、さらに浅い深さで trap する。

### 2. ツリー走査インタプリタ（プログラムの再帰）

- `src/interpreter/exec.rs`（Rust の再帰によるツリー走査）、`src/bin/nospace20.rs:195`（`interpret_with_env` を使用）

再現: `f(x){ if: x == 0 { return: 0; }; return: f(x - 1) + 1; }`

| 呼び出し深さ | nospace20 run（debug） | WS コンパイル後（whitespace20） |
|---|---|---|
| 2000 | 正常 | — |
| 3000 | stack overflow abort（rc=134、位置情報なし） | — |
| 1,000,000 | — | 正常に 1000000 を出力 |

明示スタックの NospaceVM（`interpreter/vm/`）が既にあるのに CLI では使われず、2 つのインタプリタが組み込み関数・assert・アロケータ呼び出し等の意味論を重複実装している。

## 違反している原則

- 入力境界検証、AGENTS.md の panic 方針（ユーザー入力で abort）
- フェイルセーフ設計
- DRY / SRP（実行エンジンの二重実装）

## 解決策

Cも対応する。

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `ExpressionBuilder` / `StatementBuilder` に `depth` を持たせ、上限（例: 256）超過で `CodeParseError("nesting too deep")`。パース後の木の深さも抑えられ、意味解析・コンパイラも保護される | `tree_parser` のみ、fails テスト追加 |
| B | 二項・単項演算子の連鎖をループ化（残存する再帰を確認） | `tree_parser/expression` |
| C | CLI の run モードを NospaceVM の `step()` ループに切り替え。将来 `exec.rs` を廃止 | `bin/nospace20.rs`、`lib.rs` の `interpret_*` API |
| D | ツリー走査インタプリタに呼び出し深度カウンタを持たせ、上限超過で `InterpretError::StackOverflow` | `exec.rs`、`base/error/interpret_error.rs` |
| E | NospaceVM にもフレーム数上限を設け、WASM でのメモリ際限なし増加を防ぐ | `vm/mod.rs`、`wasm_api/nospace_vm.rs` |

推奨: A（必須）+ C（中期）+ E。C を採らない場合は D。
