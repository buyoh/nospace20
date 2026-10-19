# ネストしたブロック内の関数定義・static 宣言の不具合

関数内のネストしたブロックで関数を定義したり static を宣言したりすると、run（NospaceVM）・WS の両方で誤動作する。

作成日: 2026-10-19
ステータス: 未着手

## 背景・動機

[ws-root-block-variable-address](../done-task/ws-root-block-variable-address/README.md) の設計レビューで確認された（同タスクの範囲外）。コミット 84616dd 時点。

| # | 再現（要点） | run | WS |
|---|---|---|---|
| 1 | `func: f() { { func: inner() { return: 7; } return: inner(); }; ... }` | stack overflow | 停止しない |
| 2 | `func: f() { if: 1 { static: u(20); }; ... }` | 初期化子が失われる | 初期化子が失われ、フレーム上に置かれて呼び出しをまたいで値が保持されない（`compute_static_var_offsets` は関数スコープ直下の変数しか見ない） |
| 3 | ルートレベルのブロック内に定義した関数から、そのブロックの非 static 変数を参照 | 未確認 | 未確認。`get_var_info` の fallback（`scope_depth >= len` → `Global(local_index)`）を通り、グローバル変数と重なる可能性 |

3 は、意味解析の関数境界の判定（`scope.rs` `resolve_variable_exact` の `crossed_function_boundary`）が「関数スコープ」の段でしか成り立たず、ブロックスコープ越しの参照を許す可能性があることによる。

## 設計・方針

1・3 は [block-nested-function](block-nested-function/README.md) で設計済み（A でブロック内の関数宣言をエラーにし、B で許可するとともに 3 の境界判定を直す）。2 は未設計。候補:

- 1・2: 仕様上ネストしたブロック内の関数定義・static 宣言を許すか決める（spec.md「static 変数は関数スコープ内で定義できる」）。許さないなら意味解析エラー、許すなら意味解析・両バックエンドを修正
- 3: 関数境界を越えた非 static 参照を、ブロックスコープを含めて意味解析エラーにする
