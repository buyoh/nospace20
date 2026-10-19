# NospaceVM: 再帰中に static 変数を更新すると値が失われる

`--mode run`（NospaceVM）で、再帰呼び出しの戻り値を static 変数に加算すると、呼び出し中に行われた static の更新が失われる。

作成日: 2026-10-19
ステータス: 未着手

## 背景・動機

[ws-root-block-variable-address](../done-task/ws-root-block-variable-address/README.md) のコードレビューで見つかった（同タスクの範囲外。原因は NospaceVM 側）。

```
func: rec(n) { static: c(100); let: z(0); if: n > 0 { z = rec(n - 1); }; c += z; return: c; }
func: __main() { __puti(rec(2)); }
```

| 実行 | 出力 |
|---|---|
| `--mode run`（NospaceVM） | `300` |
| WS（`--mode compile --target ws` + `whitespace20`） | `400`（手計算と一致: rec(0)=100, rec(1)=200, rec(2)=400） |

run モードは large テストの正解器として使われるため、正解器側の誤り。

## 設計・方針

未調査。推定: NospaceVM が static 変数をフレームごとに保持し、関数から戻るときに書き戻している（または `c += z` の左辺の読み出しを再帰前に行っている）。

- `src/interpreter/vm/` の static 変数の読み書き・関数の戻りを調査する
- 直接実行インタプリタ（`src/interpreter/exec.rs`）でも同じか確認する
- [retire-direct-interpreter](retire-direct-interpreter/README.md) との関係（NospaceVM を削除するなら、そのタスクで解消される）を確認し、必要なら回帰テストを `resources/tests/passes/` に追加する
