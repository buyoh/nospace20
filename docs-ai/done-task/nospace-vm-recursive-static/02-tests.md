# 02: テスト

## large テスト

`resources/tests/passes/scope/` に success_io のテストを追加し、`test-manifest.yaml` の static 関連テストの近くに登録する。除外ターゲットは指定しない（interpreter・`_vm`・opt_all・randomize・whitespace-self の全ターゲットで実行する）。

| ファイル | 内容 | 期待出力 |
|---|---|---|
| `static_recursive_update_001` | R1: 再帰の戻り値を static に加算 | `400\n` |
| `static_recursive_nested_func_001` | R2: ネストした関数の再帰から親関数の static を更新（2 回呼んで永続も確認） | `33 66\n` |
| `static_recursive_array_pointer_001` | R3: static 配列の要素をポインタ経由で再帰の後に更新 | `406\n` |
| `static_main_init_001` | `__main` の static 変数が初期化子の値を持つ（`func: __main() { static: s(7); __puti(s); }`） | `7\n` |
| `static_address_persist_001` | 非再帰: `func: p() { static: c(3); return: &c; }` で得たポインタ経由の書き込みが次の呼び出しで見える（`*x = 5; __puti(*p());`） | `5\n` |
| `static_init_calls_later_func_001` | 前に定義した関数の static 初期化子から、後ろで定義した static を持つ関数を 2 回呼ぶ（`func: h() { static: k(g(0) + g(1) * 10); ... } func: g(v) { static: m; if: v == 0 { m = 5; } else: { m += v; }; return: m; }`、`__main` で `h()` と `g(2)`）。確保順序の変更（01 のツリー走査 5）の確認 | `65 8\n` |
| `for_return_scope_001` | for ループの中から return した後、呼び出し元の変数・static が正しい（01 の NospaceVM 4 の不具合） | `742 744 1\n` |
| `for_return_scope_002` | ブロック内の for から return を 20 回繰り返した後、同じ深さで呼んだ関数のローカル変数のアドレスが変わらない（init ブロックの領域リークの検出。実装時にコードレビュー指摘で追加） | `705 710 1\n` |
| `static_recursive_nested_func_002` | 2 段ネストした関数の再帰から祖父関数の static を更新（実装時に追加） | `404 808\n` |

randomize ターゲットでも決定的になるよう、static 変数・配列は初期化子または代入で値を決めてから読む（R3 は `n == 0` のときに `a[0]`・`a[1]` に 0 を代入する）。

期待出力は手計算と WS（`--mode compile --target ws` + `whitespace20`）の結果で確認する（R1〜R3 は調査時に確認済み。`static_main_init_001` は実装時に確認する）。修正前は R1〜R3 が interpreter・`_vm` 系で失敗し、`static_main_init_001` が interpreter 系で失敗することを確認する。

## unit テスト（`src/interpreter/static_storage_tests.rs`）

`FunctionStaticStorage` は意味解析の `Scope`（`IdentifierRef` の `scope_depth` / `owning_func_index`、`variables` の `slot_index`）とアロケータに依存する自明でない規則なので、unit テストを置く。ソースを意味解析して得た `Scope` から `allocate` し、`resolve` を確認する。

| ケース | 確認内容 |
|---|---|
| 関数スコープ直下の static | 規則 3 で `base + slot` |
| ブロック内からの static 参照（`scope_depth = 1`、`scope_stack_len` が 1 多い） | 規則 3 で `base + slot` |
| 関数スコープの非 static 変数・ブロック変数（static と同じスロット番号） | None |
| `owning_func_index: Some(owner)` | 規則 2 で `base(owner) + slot` |
| 実行中の関数が無い（static 初期化子） | None |
| static 配列の 2 番目以降のスロット | `static_slots` が true |
| 2 段以上のネスト（祖父関数の static。`owning_func_index` が直近の親でない） | 規則 2 で `base(祖父) + slot` |
| static 領域が無い（`FunctionStaticStorage::new()` のまま） | None |

`IdentifierRef` は、意味解析が実際に出力したもの（関数本体の式から取り出す）を使うのが望ましいが、取り出しが煩雑な場合は `exec_tests.rs` と同様に手で組み立てる。

## NospaceVM の unit テスト（`src/interpreter/vm/tests.rs`）

`func_frames` は中断・再開をまたぐ状態なので、R1 を `step` で少しずつ（1 ステップずつ）実行しても `400` になることを確認する 1 件を追加する。

