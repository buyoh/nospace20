# WhitespaceVM: 算術オーバーフローで panic（`i64::MIN / -1` は常に abort）

- 重要度: **high**
- 状態: 未着手

## 進捗

- [ ] オーバーフロー時の仕様決定
- [ ] 修正・境界値テスト追加

## 問題点

`src/whitespace/interpreter.rs:627,632,637` の Add/Sub/Mul は素の演算子を使っており、debug ビルドでは panic、release ビルドでは黙って wrap する（ビルドプロファイル依存）。`:646,657` の Div/Mod は 0 除算のみチェックしており、`i64::MIN / -1`・`i64::MIN % -1` はビルド設定に関係なく panic する。

`Cargo.toml:58` で `panic = "abort"` のため、CLI はプロセス abort、WASM は trap してインスタンスが使えなくなる。ブラウザで信頼できないプログラムを動かす場合、1 命令でホストを落とせる。

### 再現

```
a = -9223372036854775807 - 1; b = 0 - 1; __puti(a / b);
  nospace20 run → -9223372036854775808
  whitespace20  → panicked at src/whitespace/interpreter.rs:646:28: attempt to divide with overflow
a = 9223372036854775807; __puti(a + 1);
  whitespace20  → panicked at interpreter.rs:627:58: attempt to add with overflow
```

### バックエンド間の不整合

| 実行系 | オーバーフロー時の挙動 |
|---|---|
| ツリー走査インタプリタ / NospaceVM / 定数畳み込み | wrapping（`pure_eval.rs`） |
| whitespace20 | panic、または wrap（ビルド依存） |
| wsc | 多倍長整数（オーバーフローしない） |

`docs/spec.md` にオーバーフローの規定はない。

## 違反している原則

- AGENTS.md「Panic は回復不能な異常のみ」
- フェイルセーフ設計、DoS 耐性
- バックエンド間の意味の一貫性

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `binary_op` を `wrapping_*` に統一し `pure_eval` と揃える | `whitespace/interpreter.rs` のみ。wsc とは乖離したまま |
| B | `checked_*` にし、`RuntimeError::ArithmeticOverflow` を追加してエラーとして返す | `base/error/ws_error.rs`、`bin/whitespace20.rs` のエラー表示、`wasm_api/whitespace_vm.rs` の結果変換 |
| C | 言語仕様でオーバーフロー時の挙動を定義し、全バックエンドを揃える | `docs/spec.md`、`pure_eval`、両 VM。境界値テスト追加 |

推奨: 即時対処として A または B、並行して C で仕様を確定。[09-condition-opt-overflow-semantics.md](09-condition-opt-overflow-semantics.md) と同時に検討すること。
