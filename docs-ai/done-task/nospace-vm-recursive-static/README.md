# NospaceVM: 再帰中に static 変数を更新すると値が失われる

`--mode run`（NospaceVM）とツリー走査インタプリタで、再帰呼び出しの中で行われた static 変数の更新が、外側の呼び出しから戻るときに古い値で上書きされる。static 変数をフレームへコピーする方式をやめ、static 領域へ直接アクセスする方式に直す。

作成日: 2026-10-19
ステータス: 完了

## 背景・動機

[ws-root-block-variable-address](../ws-root-block-variable-address/README.md) のコードレビューで見つかった（同タスクの範囲外。原因は NospaceVM 側）。

```
func: rec(n) { static: c(100); let: z(0); if: n > 0 { z = rec(n - 1); }; c += z; return: c; }
func: __main() { __puti(rec(2)); }
```

| 実行 | 出力 |
|---|---|
| `--mode run`（NospaceVM） | `300` |
| WS（`--mode compile --target ws` + `whitespace20`） | `400`（手計算と一致: rec(0)=100, rec(1)=200, rec(2)=400） |

run モードとツリー走査インタプリタは large テストの正解器として使われるため、正解器側の誤り。

## 調査結果

### 再現（コミット c3dd46d 時点）

| # | 再現コード（要点） | run | WS（正） | 手計算 |
|---|---|---|---|---|
| R1 | 上記 `rec(2)` | `300` | `400` | `400` |
| R2 | `func: outer() { static: s(0); func: inner(n) { s += 1; if: n > 0 { inner(n - 1); }; s += 10; return: s; }; return: inner(2); }` → `outer(), outer()` | `11 22` | `33 66` | `33 66` |
| R3 | `func: arr(n) { static: a[2]; ...; let: p(&a[1]); if: n < 3 { arr(n + 1); }; *p = *p + n; a[0] += 1; return: a[0] * 100 + a[1]; }` → `arr(0)` | `100` | `406` | `406` |

R2 はネストした関数を再帰させ、親関数の static を更新する。R3 は static 配列の要素をポインタ経由で更新する。

### 原因

NospaceVM（`src/interpreter/vm/`）とツリー走査インタプリタ（`src/interpreter/exec.rs`）は、どちらも static 変数を「コピーイン・コピーアウト」で扱っている。

1. 関数呼び出し時、フレーム（関数スコープの領域）を確保し、static 領域（`Environment.function_static_addrs[func_idx]`）から static 変数の値をフレームの同じスロットへコピーする（VM: `load_static_vars`、`src/interpreter/vm/scope.rs:73-95`。ツリー走査: `src/interpreter/exec.rs:254-276`）
2. 関数本体の static 変数へのアクセスは、フレーム上のコピーに対して行う（`resolve_addr` / `get_variable` は `scope_stack` だけを見る）
3. 関数から戻るとき、フレームのコピーを static 領域へ書き戻す（VM: `save_static_vars`、`scope.rs:49-71`、`exec.rs` / `mod.rs` の 6 箇所。ツリー走査: `exec.rs:297-318`）

再帰すると、外側の呼び出しは呼び出し前に読み込んだ古い値をフレームに持ったまま、内側の呼び出しが static 領域へ書き戻した値を上書きする（R1）。

static 変数のアドレスもフレーム上のコピーを指すため、`&a[1]` のポインタは呼び出しごとに別のアドレスになる（R3）。

ネストした関数から親関数の static 変数を参照する場合（`owning_func_index: Some(..)`）、両インタプリタは `owning_func_index` を見ず、`scope_stack` を `scope_depth` 段さかのぼった先を使う。これは「ネストした関数が親関数から直接呼ばれる」ときだけ親関数のフレーム（のコピー）を指す。ネストした関数が自分自身を再帰呼び出しすると、さかのぼった先は外側の `inner` のフレームになり、別の変数を読み書きする（R2）。

付随して、ツリー走査インタプリタの `LocalEnvironment::new_func`（`exec.rs:67-93`。`__main` と `interpret_func` の入口）はコピーインを行わないため、`__main` 内の static 変数は初期化子の値を読めない（`__main` は 1 回しか呼ばれないので、現状のテストでは表面化していない）。

WS コンパイラ（`src/compiler_ws/context.rs:228-270` の `get_var_info`）は static 変数を static 領域へ直接アクセスしており、正しい。

## 設計・方針

| 項目 | 方針 | 詳細 |
|---|---|---|
| static 変数のアクセス | コピーイン・コピーアウトを廃止し、static 変数への参照を static 領域のアドレスへ直接解決する（WS コンパイラの `get_var_info` と同じ規則） | [01-direct-static-access.md](01-direct-static-access.md) |
| 共通部品 | 関数ごとの static 領域のベースアドレスと static スロットの表を `FunctionStaticStorage`（新規 `src/interpreter/static_storage.rs`）にまとめ、両インタプリタで共有する。`Environment.function_static_addrs` を置き換える | 同上 |
| 対象 | NospaceVM とツリー走査インタプリタの両方 | 同上 |
| テスト | large テスト（R1〜R3）と、`FunctionStaticStorage` の解決規則の unit テスト | [02-tests.md](02-tests.md) |

### 両方のインタプリタを直す理由

[retire-direct-interpreter](../../task/retire-direct-interpreter/README.md)（設計完了・未実装）は NospaceVM を削除し、ツリー走査インタプリタを意味解析テストの正解器（`src/semantic_test/`）として残す。

- ツリー走査インタプリタは同タスク後も正解器として残るため、修正が必要
- NospaceVM は同タスクで削除されるが、それまでは `--mode run` と `_vm` large テストの実行系である。large テストに R1〜R3 を追加すると `_vm` ターゲットも実行されるので、除外指定を書くより直すほうが単純（修正は `resolve_addr` と関数の出入りに限られる）

retire-direct-interpreter の 03（`exec.rs` の評価ロジックは変更しない）とは矛盾しない。本タスクが先に入れば、移設されるのは修正後の `exec.rs` になる。新しく追加する `static_storage.rs` / `static_storage_tests.rs` は同タスク 03 の移設一覧に追記する（VM 削除後はツリー走査専用になる）。

付随して、NospaceVM で for ループの中から return すると `scope_stack` がずれる既存の不具合も直す（本タスクの解決規則の前提となる不変条件のため。[01](01-direct-static-access.md) の NospaceVM 4）。

## タスクステップ・チェックリスト

1. [x] `FunctionStaticStorage` を追加し、`Environment.function_static_addrs` を置き換える（[01](01-direct-static-access.md)）
2. [x] ツリー走査インタプリタ: 関数フレームの記録、static 参照の直接解決、コピーイン・コピーアウトの削除、static 領域の確保と初期化の分離（[01](01-direct-static-access.md)）
3. [x] NospaceVM: 関数フレームの記録、static 参照の直接解決、`save_static_vars` / `load_static_vars` の削除（[01](01-direct-static-access.md)）
4. [x] テスト追加（[02](02-tests.md)）
5. [x] retire-direct-interpreter 03 の移設一覧に `static_storage.rs` / `static_storage_tests.rs` を追記
6. [x] VERSION.md の WIP に記載
7. [x] `cargo test` の実行と結果の記録

## 進捗

- 実装: `src/interpreter/static_storage.rs`（`FunctionStaticStorage` / `FuncFrame`）を追加し、両インタプリタを直接解決に変更。NospaceVM の関数出口は `leave_func_scope` で `scope_index` まで `scope_stack` を戻す
- テスト: large テスト 9 件（02 の表）、`static_storage_tests.rs` 9 件、`vm/tests.rs` の `test_step_one_at_a_time_recursive_static`
  - `static_init_calls_later_func_001` は設計時の例（`k(g() * 10 + g())` で `g` が `static: c(0)` を持つ）が、`g` の初期化子より前に `c` を読む未定義動作（wsc は未初期化ヒープ参照でエラー）だったため、`g` が static を代入してから読む形に変えた（期待値 `65 8`）
  - 修正前: 新規 large テストのうち 17 件が失敗（interpreter / opt_all / `_vm`）。修正後: `cargo test` 全件成功、wsc 連携（`--ignored`）の新規 10 件も成功
- コードレビュー（1 回）の指摘への対応: ブロック内 for からの return で init ブロックの領域がリークする件を `propagate_flow` で修正し `for_return_scope_002` を追加、`static_slots` に `debug_assert`、`BlockCompletion` の doc 修正、unit テストのコメント追加。指摘の修正は小さく、テストで確認したため再レビューは省略
- 対応しなかった指摘（既存の挙動・低優先）: ツリー走査の `resolve_address` のフォールバック（規則 4）は `scope_stack.len() - 1 - scope_depth` を素の減算で計算し、範囲外で panic する（VM は `saturating_sub`）。本タスクの変更ではなく、retire-direct-interpreter で `semantic_test` に移すときに揃える。`owning_func_index` の unit テストは手組みの `IdentifierRef` を使う（意味解析が出す値は large テスト `static_recursive_nested_func_002` で end-to-end に確認）

