# WS コンパイラ: 関数フレーム外のブロック変数・ブロック内の static 変数のアドレス誤り

WS コンパイラで、関数フレームを持たない場所（ルートレベル・関数内 static 初期化子）のブロック変数が予約されていないアドレスに置かれ、グローバル変数・static 変数・アロケータ領域を上書きする。設計中に、関数内のブロックから同じ関数の static 変数を参照すると誤ったアドレスになるバグも見つかったため、同じタスクで直す。

作成日: 2026-10-19
ステータス: 完了

## 背景・動機

[array-size-overflow](../../task/array-size-overflow/README.md) のレビュー中に見つかった（同タスクの範囲外）。

コミット 84616dd 時点で、`nospace20 --mode compile --target ws x.ns -o x.ws && whitespace20 x.ws` と `nospace20 --mode run x.ns`（NospaceVM、正しい結果）を比べた結果:

| # | 再現コード（要点） | run | WS | 原因 |
|---|---|---|---|---|
| A1 | `let: g; { let: b; b = 5; }; func: __main() { __puti(g); }` | `0` | `5` | ルートのブロック変数 `b` が `g` と同じアドレス |
| A2 | `let: g(1); let: arr[3]; if: 1 { let: t[2]; t[0] = 100; t[1] = 200; };` → `g, arr[0], arr[1]` | `1 0 0` | `100 200 0` | ルートの `if` ブロックも同じ |
| A3 | `let: g(1); static: h({ let: t; t = 9; t; }); let: k({ let: u; u = 4; u; });` → `g, h, k` | `1 9 4` | `1 4 4` | ルートの static 初期化子のブロック変数 `t`→`g`、`u`→`h` |
| B1 | `let: g; let: h; func: f() { let: a; let: b; static: s({ let: t; t = 7; t; }); ... }` | — | 値は正しい | 関数内 static 初期化子のブロック変数 `t` が `GLOBAL_PTR + 3`（グローバル領域の外。FSBA のフリーリスト表）に書かれる |
| C1 | `func: f() { static: s(3); { s += 1; }; if: 1 { s += 10; }; return: s; }` → `f(), f()` | `14 25` | `3 3` | ブロック内から static `s` を参照すると、static 領域でなくフレーム上のアドレスになる |
| C2 | `func: f() { static: s(3); { let: t; t = 5; }; return: s; }` | `3` | `5` | ブロック変数 `t` が、同じスロット番号の static `s` と誤認される |
| C3 | `let: g(1); func: f() { static: s1(4); static: s2({ let: t; t = s1 + 1; t; }); return: s2; }` → `g, f()` | `1 5` | `1 1` | static 初期化子のブロックから `s1` を参照（C1 と同じ）。B1 の書き込み先も重なる |
| B2 | `static: g(9); let: h(2); func: f() { let: a(5); static: s({ a = 7; 1; }); ... }` → `g, h` | `9 2` | `7 2` | static 初期化子から関数スコープ直下の非 static 変数 `a` に書くと `GLOBAL_PTR + slot`（= `g`）に書かれる。読む場合（`static: s(a);`）も `g` の値になる（run は `0`） |
| B3 | `func: tmpl(x), alias: constexpr: step { static: s({ let: t; t = step; t; }); ... }` の 2 インスタンス | `1 2 3 4` | `1 2 4 4` | テンプレートのインスタンスごとの static 初期化子（B1 と同じ） |
| A4 | `let: g(1); namespace: N { let: x(5); { let: y; y = 9; }; }` → `g` | `1` | `9` | namespace 内のルートブロック（A1 と同じ） |

### 原因

**A（ルートレベル）**: グローバルの `CodeGenContext` は `scope_offsets = [0]`、`next_var_offset = 0` で始まる（`src/compiler_ws/context.rs:124-125`）。ルートのブロックは `enter_block_scope`（`context.rs:322-325`）でオフセット 0 から積まれ、`VarScope::Local` として `heap[LOCAL_HEAP_BEGIN] + offset` でアクセスされる。グローバル実行中の `heap[LOCAL_HEAP_BEGIN]` は `GLOBAL_PTR`（`alloc_runtime/bump.rs:20-25`、`fsba.rs:650-656`）なので、グローバル変数と同じアドレスになる。`global_heap_size()`（`context.rs:197-199`）はこの領域を数えていない。

**B（関数内 static 初期化子）**: `enter_function_for_static_init`（`context.rs:169-194`）は `scope_offsets = [0]`、`next_var_offset = func_scope_var_count` で始まる。フレームは確保しないので `heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR` のまま。結果、関数スコープ直下の非 static 変数は `GLOBAL_PTR + slot`（B2）、ブロック変数は `GLOBAL_PTR + func_scope_var_count + k` に置かれ、グローバル・static 変数、またはグローバル領域の直後（FSBA の管理領域の先頭 = サイズクラス表）と重なる。`calculate_total_variable_count`（`statement.rs:418-`）は `static_init_statements` を数えず、どの領域にも予約されていない。

**C（ブロック内からの static 参照）**: `get_var_info`（`context.rs:220-270`）は、static 変数の検索を `var_ref.scope_depth == 0 || scope_depth >= scope_offsets.len()` のときに行う。`scope_depth` は「現在のスコープから何段外か」なので、`scope_depth == 0` は「関数スコープ直下」ではなく「現在のブロック」を表す。

- ブロックの中から関数スコープの static を参照すると `scope_depth >= 1` かつ `< len` になり、static の検索を通らず、フレーム（`scope_offsets[0] + slot`）として扱われる（C1・C3）
- ブロック変数（`scope_depth == 0`）の `local_index` が、関数スコープの static のスロット番号と一致すると、static と誤認される（C2）

C は A・B と原因箇所が違うが、「関数フレーム外・ブロック内の変数アドレス」という同じ領域の問題で、B の修正の検証（C3）にも C の修正が必要なため、このタスクに含める。

## 設計・方針

| 項目 | 方針 |
|---|---|
| A・B の直し方 | ルートの初期化文に含まれるブロック変数の領域と、関数内 static 初期化子が使う「フレーム相当」の領域（関数スコープ直下の変数 + 初期化子内のブロック変数）を、グローバル領域の末尾に予約する。詳細は [01](01-init-block-region.md) |
| 領域の再利用 | しない。ブロックごとに一意のオフセットにする（関数フレームの `next_var_offset` を戻さない方針と同じ）。ルート用と関数ごとの static 初期化子用を別々に積む |
| C の直し方 | static の検索条件を「関数スコープ直下の変数（`scope_depth == scope_offsets.len() - 1`）」または「親関数の static（`owning_func_index` が `Some`）」に直す。詳細は [02](02-static-var-in-nested-block.md) |
| グローバル領域の配置の計算 | `compute_static_var_offsets`（`context.rs:26-42`）を、static 変数と初期化文のブロック領域をまとめて計算する `GlobalLayout`（新モジュール `src/compiler_ws/global_layout.rs`）に置き換える |
| `GlobalLayout` の渡し方 | `compile_with_options`（`mod.rs`）で計算し、`&'a GlobalLayout` として `CodeGenContext` に注入する（`alloc_runtime: &'a dyn AllocRuntime` と同じ形） |
| ブロック変数の数え方 | `statement.rs` の `calculate_total_variable_count`・`count_nested_vars_*` を新モジュール `src/compiler_ws/block_var_count.rs` に移し、関数フレームと `GlobalLayout` で共有する |
| 直接実行インタプリタ | 対象外（正しく動作している。[retire-direct-interpreter](../../task/retire-direct-interpreter/README.md) で廃止予定） |
| オーバーフロー検査 | このタスクでは追加しない。[array-size-overflow/01](../../task/array-size-overflow/01-checked-size-arithmetic.md) が対象にしている `compute_static_var_offsets`・`global_heap_size`・`enter_block_scope` の検査箇所が `GlobalLayout` に移るので、同ドキュメントの対象箇所を更新する |

### 対象外（記録のみ）

以下は設計レビューで確認された別のバグ。run・WS の両方で誤動作し、原因も別（意味解析・関数定義の生成）なので、別タスク [nested-block-function-and-static](../../task/nested-block-function-and-static.md) に記録する。本タスクのテストはこれらの構文を使わない。

- ネストしたブロックの中に関数を定義する（`func: f() { { func: inner() { return: 7; } return: inner(); }; }`）と、run は stack overflow、WS は停止しない
- ネストしたブロックの中で static を宣言する（`if: 1 { static: u(20); };`）と、両モードで初期化子が失われる。WS ではフレーム上に置かれ、呼び出しをまたいで値が保持されない
- ルートレベルのブロック内に定義した関数から、そのブロックの非 static 変数を参照する。意味解析は関数境界を「関数スコープ」でしか判定しないため、参照を許してしまう可能性がある（未確認）

なお B2 の「static 初期化子から同じ関数の非 static 変数を読む」は、run では `0`（初期化前）になる。仕様上エラーにすべきかは未定義だが、本タスクでは WS を run と同じ `0` にする（01 で予約領域に置く）ところまでを行う。

## タスクステップ・チェックリスト

1. [x] 01: 初期化文のブロック変数領域の予約（[01-init-block-region.md](01-init-block-region.md)）
   1. [x] `block_var_count.rs` を作り、`statement.rs` の数え上げ関数を移す
   2. [x] `global_layout.rs`（`GlobalLayout`・`StaticInitFrame`）を作り、`compute_static_var_offsets` を置き換える
   3. [x] `mod.rs` で `GlobalLayout` を計算し `CodeGenContext` に注入。ルート・static 初期化コンテキストのオフセットを `GlobalLayout` から取る。`global_heap_size()` を `GlobalLayout::total_size()` にする
   4. [x] `debug_assert!`（ルート・static 初期化・関数フレーム）
2. [x] 02: ブロック内からの static 変数参照の修正（[02-static-var-in-nested-block.md](02-static-var-in-nested-block.md)）
3. [x] テスト追加（large 7 件、unit 5 件。新しい large テストは修正前のコードで WS 系が失敗することを確認済み）
4. [x] 修正箇所にバグの原因と解決策のコメントを記載
5. [x] VERSION.md 追記
6. [x] 別タスク [nested-block-function-and-static](../../task/nested-block-function-and-static.md) を作成
7. [x] [array-size-overflow/01](../../task/array-size-overflow/01-checked-size-arithmetic.md) の対象箇所を更新

## 進捗

- 2026-10-19: 設計・設計レビュー（1 回）完了。レビュー指摘で、static 初期化子から非 static 変数を参照するケース（B2）・テンプレート（B3）・namespace（A4）を範囲に追加し、`GlobalLayout` は `&'a` で注入する形に変更
- 2026-10-19: 実装。`cargo test` 全件成功（`--ignored` の wsc 系も新テスト分は成功）。実装中の設計変更は 01 に記録（static 初期化子のフレーム相当領域を非 static 変数の末尾までに縮小）
- 2026-10-19: コードレビュー（1 回）。正しさの指摘なし。minor 指摘のうち、debug_assert の `map_or` の除去・`count_nested_vars_in_statements` の doc 追加・回帰ガードテストの manifest コメント・フレーム相当領域が未参照の非 static 変数の分も確保することの doc 訂正と unit テスト追加を対応。領域の縮小は [ws-static-init-frame-compaction](../../task/ws-static-init-frame-compaction.md)、レビューで見つかった NospaceVM の再帰 static のバグは [nospace-vm-recursive-static](../../task/nospace-vm-recursive-static.md) に分離
