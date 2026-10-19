# 01: static 変数への直接アクセス

static 変数のコピーイン・コピーアウトを廃止し、static 変数への参照を static 領域のアドレスへ直接解決する。NospaceVM とツリー走査インタプリタの両方に適用する。

## 解決規則

WS コンパイラの `get_var_info`（`src/compiler_ws/context.rs:228-270`）と同じ規則にする。`IdentifierRef` は次の順に解決する。

| # | 条件 | 解決先 |
|---|---|---|
| 1 | `is_global` | 従来どおり `global_base_addr + local_index` |
| 2 | `owning_func_index: Some(owner)`（ネストした関数から親関数の static を参照） | `static_base(owner) + local_index` |
| 3 | `owning_func_index: None`、実行中の関数があり、参照先が「実行中の関数の関数スコープ」で、そのスロットが static 変数 | `static_base(current_func) + local_index` |
| 4 | それ以外 | 従来どおり `scope_stack[len - 1 - scope_depth] + local_index` |

- 「参照先が関数スコープ」は、`scope_stack.len() - 1 - scope_depth` が、関数を呼んだときに記録した関数スコープの位置（`scope_stack` 上のインデックス）と一致することで判定する。ブロックの中から参照した場合（`scope_depth >= 1`）も正しく判定できる（WS コンパイラの C1 修正と同じ考え方）
- `local_index` は関数スコープのスロット番号である（配列の場合は先頭スロット）。static 配列 `a[2]` の要素アクセス・`&a[1]` は、解決した先頭アドレスに添字を足すので、static 領域の要素を直接指す
- static 領域が無い（`interpret_func` を `interpret_global` なしで呼んだ場合など）ときは 2・3 に該当しないものとして 4 に進む。現状の「コピーイン無し」の挙動と同じで、既存の利用（`lib.rs` の `interpret_func*`）を壊さない
- 規則 3 の位置計算は `scope_stack_len.checked_sub(1 + scope_depth)` とし、範囲外なら `None`（規則 4 に進む。規則 4 側の挙動は従来どおり）

### 前提とする不変条件

規則 3 は「関数の実行中、その関数スコープは `scope_stack[FuncFrame.scope_index]` にある」ことを前提とする。関数から出るときは `scope_stack` を `scope_index` の長さまで戻す（後述の NospaceVM 4）。

### static 初期化子の実行中

static 初期化子は、関数呼び出しの外で `scope_stack = [static_base]`（ツリー走査は新しい `LocalEnvironment`、VM は `ExecFuncStaticStmt` で push）として実行する。実行中の関数は無いので、規則 3 は適用されず、規則 4 で `static_base + slot` に解決される。これは従来どおりで正しい（初期化子から同じ関数の別の static を参照する場合も含む）。初期化子の中で関数を呼ぶと、その呼び出しの中は規則 3 が適用される。

## `FunctionStaticStorage`（新規 `src/interpreter/static_storage.rs`）

関数ごとの static 領域（ベースアドレスと、どのスロットが static か）を保持し、上の規則 2・3 を判定する。両インタプリタで共有する。`Environment.function_static_addrs: BTreeMap<usize, i64>` を置き換え、`Environment.function_statics: FunctionStaticStorage` とする。

```rust
/// 関数内 static 変数の領域（関数ごとのベースアドレスと static スロットの表）
///
/// static 変数への参照を static 領域のアドレスへ解決する。NospaceVM とツリー走査インタプリタで共有する。
pub(crate) struct FunctionStaticStorage {
    /// 関数インデックス → static 領域（static 変数を持たない関数は None）
    regions: Vec<Option<StaticRegion>>,
}

/// 1 関数分の static 領域
struct StaticRegion {
    /// static 領域のベースアドレス（アロケータ上。関数スコープの variable_count 分）
    base_addr: i64,
    /// 関数スコープのスロットごとに、static 変数のスロットか（配列は全要素のスロットを true）
    static_slots: Vec<bool>,
}

/// 実行中の関数呼び出し
#[derive(Clone, Copy)]
pub(crate) struct FuncFrame {
    pub func_idx: usize,
    /// 関数スコープの領域が積まれている `scope_stack` 上のインデックス
    pub scope_index: usize,
}

impl FunctionStaticStorage {
    pub fn new() -> Self;
    /// static 変数を持つ全関数の static 領域を確保する（初期化子は実行しない）
    pub fn allocate(scope: &Scope, allocator: &mut InterpreterAllocator, randomize_uninit: bool) -> Self;
    pub fn base_addr(&self, func_idx: usize) -> Option<i64>;
    /// 規則 2・3 に該当すれば static 領域のアドレスを返す。該当しなければ None（呼び出し側が規則 4 で解決する）
    pub fn resolve(&self, id: &IdentifierRef, current: Option<FuncFrame>, scope_stack_len: usize) -> Option<i64>;
}
```

- `static_slots` は `func.block.scope.variables` のうち `is_static` のものから作る（`slot_index .. slot_index + array_size.unwrap_or(1)`）。アクセスごとに `variables` を走査しないよう、確保時に一度だけ作る
- 確保（`allocate`）は両インタプリタで同じ処理なのでここにまとめる。領域の大きさは従来どおり関数スコープの `variable_count`

## ツリー走査インタプリタ（`src/interpreter/exec.rs`、`mod.rs`）

1. `LocalEnvironment` に `func_frames: Vec<FuncFrame>` を追加する
2. `resolve_address` に規則 2・3 を加える（`self.env.function_statics.resolve(id, self.func_frames.last().copied(), self.scope_stack.len())`）。`get_variable` / `set_variable` は同じアドレス計算を重複して持っているので、`resolve_address` を呼ぶ形にまとめる
3. `interpret_call_user_function_by_ref`: フレームを push する直前に `FuncFrame { func_idx: func_ref.local_index, scope_index: scope_stack.len() }` を push し、関数スコープを pop するときに pop する。static のコピーイン（`exec.rs:254-276`）・コピーアウト（`exec.rs:297-318`）は削除する
4. `LocalEnvironment::new_func` の引数を `func: &Function` から `func_idx: usize` に変え、`func_frames = [FuncFrame { func_idx, scope_index: 0 }]` で始める。呼び出し元（`interpret_func`、`interpret_all`、`exec_tests.rs` の 2 箇所）を合わせる。`interpret_func` は名前から関数インデックスを引く必要があるので、`Scope` に `get_function_index(name) -> Option<usize>`（`symbol_table.function_name_to_index` を引く）を追加する
5. `interpret_global`: 全関数の static 領域の確保を、ルートの static 初期化子より前に行う（`FunctionStaticStorage::allocate`）。`initialize_function_statics` は初期化子の実行だけにする。順序は NospaceVM の `AllocGlobals` と同じになる（グローバル領域 → 全関数の static 領域 → ルート static 初期化子 → 関数 static 初期化子 → ルート文 → `__main`）
   - 従来は「関数 f の static 領域の確保・初期化」を関数ごとに行い、`function_static_addrs` への登録は初期化子の後だった。そのため、ルートの static 初期化子や先に初期化される関数の初期化子から、まだ領域の無い関数を呼ぶと、その関数の static は毎回フレーム上の未初期化値になっていた。直接解決では領域が先に必要なので、確保を前に出す

付随して、`__main` の static 変数も static 領域へ解決されるようになる（調査結果の「付随して」の不具合が直る）。

## NospaceVM（`src/interpreter/vm/`）

1. `NospaceVM` に `func_frames: Vec<FuncFrame>` を追加する
2. `resolve_addr`（`scope.rs:8-16`）に規則 2・3 を加える。`get_variable` / `set_variable` は既に `resolve_addr` を使っている
3. `push_func_frame`（`mod.rs:692-`）: `scope_stack.push(scope_addr)` の前に `FuncFrame { func_idx, scope_index: scope_stack.len() }` を push する。`load_static_vars` 呼び出しを削除する
4. 関数フレームから出る 6 箇所（`exec.rs` の `finish_exec_block` の `MainFunc` / `UserFunc`、`mod.rs` の flow 処理の `MainFunc` / `UserFunc` の return あり・なし）の `save_static_vars + leave_scope` を、新しいヘルパ `leave_func_scope()` に置き換える。`leave_func_scope` は `func_frames.pop()` で得た `scope_index` まで `scope_stack` を pop し、pop した各領域を `free_internal` する（関数スコープと、関数内で残っていたブロックの領域）。`BlockCompletion::MainFunc` / `UserFunc` の `func_idx` は使わなくなるので削除する（`scope_addr` は `leave_func_scope` 内の `debug_assert` で関数スコープと一致することの確認にだけ使う）
   - 既存の不具合の修正を兼ねる: for ループの中から `return` すると、`propagate_flow`（`mod.rs:486-534`）は `ForLoop` フレームを pop するだけで、`GlobalStmts` として積んだ init ブロックの領域を `scope_stack` から外さない。続く `leave_scope(scope_addr)` は末尾が一致しないので関数スコープも残り、以降の変数解決がずれる（`func: f() { for: { let: j(0); } { j < 5; } { j += 1; } { if: j == 2 { return: j; }; }; return: 99; }` を呼んだ後に呼び出し元の変数を読むと `invalid memory access`。WS とツリー走査は正しい）。`scope_index` まで戻すことで、関数の出口でこの不変条件を保証する
   - 実装時の追加（コードレビュー指摘）: for がブロックの中にあると、外側ブロックの `leave_scope` も末尾不一致で pop されず、`leave_func_scope` の巻き戻しだけでは内部アドレス空間がリークする（二重解放も起きる）。根本原因として `propagate_flow` で `Frame::ForLoop` を pop するとき、init ブロックの領域を `leave_scope` で外す（`ForPhase::init_scope_addr`。break 時の `WaitBody` と同じ処理）。`leave_func_scope` の巻き戻しは安全網として残す
5. `save_static_vars` / `load_static_vars`（`scope.rs:47-95`）を削除する
6. `AllocGlobals` の static 領域の確保を `FunctionStaticStorage::allocate` に置き換える。`ExecFuncStaticInits` の `function_static_addrs.get(&fi).unwrap()` は `function_statics.base_addr(fi).expect(..)` にする（`allocate` 済みという内部の不変条件なので panic でよい。その旨をコメントに書く）
7. `func_frames` は VM の中断・再開（`step`）をまたいで保持される状態であり、`frames` と同じく `NospaceVM` のフィールドに置く

## 範囲外

- ブロック内で宣言した static（`if: 1 { static: x(5); ... }`）とブロック内の関数宣言は、どのバックエンドでも壊れている。[nested-block-function-and-static.md](../../task/nested-block-function-and-static.md) と [block-nested-function](../../task/block-nested-function/README.md) で扱う。本タスクの `static_slots` は関数スコープ直下の static だけを対象とする
- ネストした関数から親関数の「ブロック」の変数を参照する場合（`owning_func_index: None` で規則 4 になり、呼び出し元のフレームを指す）は現状と同じ誤りのまま残る。block-nested-function 案 B を実装するときに、`FunctionStaticStorage` の解決規則を見直す

## 採用しなかった案

| 案 | 不採用の理由 |
|---|---|
| コピーイン・コピーアウトを残し、再帰呼び出しの前後で static を書き戻す・読み直す | 呼び出しのたびに全 static をコピーする必要があり、ポインタ（R3）とネスト関数の再帰（R2）は直らない |
| 意味解析で static 変数への参照を専用の識別子（例: `IdentifierRef` に `is_static` を追加）にする | WS コンパイラの解決規則も変える必要があり、修正範囲が意味解析・コンパイラに広がる。retire-direct-interpreter でインタプリタの片方は削除されるので、インタプリタ側で WS コンパイラと同じ規則を実装するほうが小さい |
| NospaceVM は retire-direct-interpreter で削除されるので直さない | 削除まで `--mode run` と `_vm` large テストの実行系であり、追加する large テストに除外指定が必要になる。修正は小さい（README「両方のインタプリタを直す理由」） |
