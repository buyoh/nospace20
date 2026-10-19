# WS コンパイラ: static 初期化子のフレーム相当領域の縮小

関数内 static 初期化子用に予約するフレーム相当領域を、初期化子が実際に参照する非 static 変数の分だけにする。

作成日: 2026-10-19
ステータス: 未着手

## 背景・動機

[ws-root-block-variable-address](../done-task/ws-root-block-variable-address/README.md) で、関数内 static 初期化子が使う変数の領域をグローバル領域の末尾に予約した（`GlobalLayout::compute`、`src/compiler_ws/global_layout.rs`）。

初期化子の無い `static: s;` も `static_init_statements` を生成するため、static 変数と引数・非 static 変数を両方持つ関数では、初期化子が非 static 変数を参照しなくても、その分（`max(slot_index + array_size)`）を確保する。例: `func: f(a, b) { let: arr[3]; static: s; }` で 5 スロット。

WS のヒープは疎（`HashMap`）なので実メモリは消費しない。グローバル領域のサイズ（FSBA の `managed_start`、Bump の `LOCAL_HEAP_END` の初期値）が増えるだけで、正しさには影響しない。優先度は低い。

## 設計・方針

未設計。候補（コードレビューの提案）:

- `static_init_statements` を走査し、関数スコープ直下の非 static 変数を参照する `Variable` / `ArrayAccess` 等（ネストしたブロック内では `scope_depth` が段数ぶん大きくなる）を集め、その `max(slot_index + size)` を `frame_var_count` にする。参照が無ければ 0
- unit テスト: `func: f(x) { static: s; }` → `total_size == 1`（`global_layout_tests.rs` の `test_static_init_frame_reserves_non_static_vars_even_if_unreferenced` を置き換える）
