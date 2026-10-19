# 01: 初期化文のブロック変数領域の予約

ルートの初期化文に含まれるブロック変数と、関数内 static 初期化子が使う変数の領域を、グローバル領域の末尾に予約する（README の A・B）。

## 設計・方針

### グローバル領域の配置

`GLOBAL_PTR` からの相対オフセット:

```
[0, V)                     グローバル変数（scope.variable_count）                     … 既存
[V, V+S)                   関数内 static 変数（compute_static_var_offsets）             … 既存
[V+S, V+S+R)               ルート初期化文のブロック変数                                … 新規
[V+S+R, V+S+R+N_f0) ...    関数 f ごとの static 初期化子のフレーム相当領域（関数順）    … 新規
total_size = V + S + R + Σ N_f
```

- `R` = `count_nested_vars_in_statements(&scope.static_init_statements) + count_nested_vars_in_statements(&scope.root_statements)`
  - ルートの static 初期化（`generate_scope` の ①）とルートの文（③）は同じグローバルの `CodeGenContext` で生成され、`next_var_offset` が通しで増えるので、1 つの領域にまとめる
  - ルートの変数は `is_global` で `GLOBAL_PTR + local_index` に解決されるので、ルートスコープ自身の変数は数えない
  - namespace 内のブロックもルートの文に含まれる（README A4）
- `N_f` = `frame_var_count + count_nested_vars_in_statements(&func.block.scope.static_init_statements)`。`static_init_statements` が空の関数は領域を持たない
  - `frame_var_count` は、関数スコープ直下の **非 static** 変数の末尾スロット（`max(slot_index + array_size)`、無ければ 0）。関数フレームの先頭と同じ並びで、static 初期化子から非 static 変数（`let: a; static: s(a);`）を参照したときの読み書き先になる（README B2）。スロット番号をそのまま使うため、途中の static 変数のスロットは詰めない
  - 実装時の変更: 当初は `func.block.scope.variable_count` 全体を確保する設計だったが、初期化子の無い `static: s;` も `static_init_statements` を生成するため、static 変数を持つ全関数で static 配列の分まで余分に確保してしまう。unit テストで判明し、非 static 変数の末尾までに変更した。static 変数を持たない関数・非 static 変数を持たない関数の `total_size` は従来と変わらない。ただし初期化子が実際に非 static 変数を参照するかは見ないため、`func: f(a, b) { let: arr[3]; static: s; }` のように static 変数と引数・非 static 変数を両方持つ関数では、その分（例では 5 スロット）グローバル領域が増える（コードレビューで指摘。WS のヒープは疎なので確保は起きず、アドレス空間が余るだけとして許容。参照される変数だけ確保する改善は [ws-static-init-frame-compaction](../../task/ws-static-init-frame-compaction.md) に記録）
  - `func.is_unused()` の関数も数える（static 変数の配置 `compute_static_var_offsets` も全関数を数えており、それと揃える。未使用関数の static 初期化子は生成されないので、領域が余るだけ）
  - ネストした関数・テンプレートのインスタンスも `scope.functions` に並ぶので、関数ごとに領域を持つ（README B3）
- `count_nested_vars_in_statements` は、文の中のブロック（`{}`・`if`・`for` の init/cond/step/body・`while`）の `variable_count` の合計で、文の属するスコープ自身の変数は数えない
- `next_var_offset` は戻さない（既存の方針）ので、ブロックが消費するオフセットの合計は数え上げた値と一致する。`enter_block_scope` は `generate_block`（`statement.rs:70`）と `generate_for_statement` の init（`statement.rs:252`）からだけ呼ばれ、どちらも数え上げに含まれる。関数フレームと同じ前提に乗る

`total_size` は `global_heap_size()` として `generate_memory_init` に渡り、Bump の `LOCAL_HEAP_END` 初期値、FSBA の管理領域の先頭（`managed_start`）が新しい領域の後ろになる。Bump・FSBA とも変わるのはこの値だけなので、どちらのアロケータでも同じ配置になる。

ブロックごとに一意のオフセット（再利用しない）にする理由: ルートのブロック変数のアドレスを取って（`&b`）ブロックの外へ持ち出すコードがあっても、他の変数と重ならない。領域の大きさは初期化文に書いた変数の数だけで、通常は小さい。

### 前提: 初期化中の `heap[LOCAL_HEAP_BEGIN]`

ルート・static 初期化コンテキストのブロック変数は、これまでどおり `VarScope::Local`（`heap[LOCAL_HEAP_BEGIN] + offset`）でアクセスする。初期化中は `heap[LOCAL_HEAP_BEGIN] == GLOBAL_PTR`（`generate_memory_init` で設定）であることを前提にする。初期化子の中で関数を呼んでも、関数プロローグが元の値を退避しエピローグが戻す（`alloc_runtime/mod.rs` `generate_common_prologue` / epilogue）ので、呼び出し後も成り立つ。large テストで「ルートのブロック内で関数を呼んだ後にブロック変数を使う」ケースを確認する。

`VarScope::Global`（絶対アドレス）で解決する案は、フレームを持たないコンテキストを区別するフラグが必要になり変更が広がるので採らない。

### 新モジュール

#### `src/compiler_ws/block_var_count.rs`

`statement.rs:418-480` の `calculate_total_variable_count`・`count_nested_vars_in_statements`・`count_nested_vars_in_statement`・`count_nested_vars_in_expression` をそのまま移す。`calculate_total_variable_count` と `count_nested_vars_in_statements` を `pub(super)` にする。`statement.rs` はこのモジュールを使う。

#### `src/compiler_ws/global_layout.rs`

```rust
/// グローバル領域（GLOBAL_PTR からの相対オフセット）の配置
///
/// グローバル変数・関数内 static 変数・初期化文が使う変数の配置を、
/// コード生成の前に一度だけ計算する。
pub struct GlobalLayout {
    /// 関数内 static 変数のオフセット
    /// キー: (関数インデックス, スロットインデックス)
    static_var_offsets: HashMap<(usize, usize), i64>,
    /// ルート初期化文のブロック変数領域 [begin, end)
    root_init_block_range: Range<i64>,
    /// 関数内 static 初期化子のフレーム相当領域
    /// キー: 関数インデックス（static 初期化子を持たない関数は含まない）
    static_init_frames: HashMap<usize, StaticInitFrame>,
    /// グローバル領域の合計サイズ（スロット数）
    total_size: i64,
}

impl GlobalLayout {
    pub fn compute(scope: &Scope) -> Self;
    pub fn static_var_offset(&self, func_index: usize, slot_index: usize) -> Option<i64>;
    pub fn root_init_block_range(&self) -> Range<i64>;
    /// StaticInitFrame { frame_base, block_base, end }
    pub fn static_init_frame(&self, func_index: usize) -> Option<StaticInitFrame>;
    pub fn total_size(&self) -> i64;
}
```

`compute_static_var_offsets`（`context.rs:23-42`）の処理は `GlobalLayout::compute` に移し、`context.rs` から削除する。

### `GlobalLayout` の注入（`mod.rs`）

`compile_with_options` で `GlobalLayout::compute(scope)` を計算し、`CodeGenContext::new_with_options(scope, &layout, debug_ext, alloc_ext, alloc_runtime)` に渡す。`CodeGenContext` は `layout: &'a GlobalLayout` を持ち、子コンテキストは参照をコピーする（今の `HashMap` の clone をやめる）。`alloc_runtime: &'a dyn AllocRuntime` と同じ注入の形。

### `CodeGenContext` の変更（`context.rs`）

| 箇所 | 変更 |
|---|---|
| フィールド `static_var_global_offsets`・`static_var_total_size` | 削除し、`layout: &'a GlobalLayout` に置き換える |
| `new_with_options` | `layout` 引数を追加。`next_var_offset = layout.root_init_block_range().start` |
| `enter_function` | `layout` を渡す以外は変更なし |
| `enter_function_for_static_init` | 引数を `(func_index, func_scope)` にする（未使用の `_total_var_count`、不要になる `func_scope_var_count` を削除）。`frame = layout.static_init_frame(func_index)` とし、`scope_offsets = [frame.frame_base]`、`next_var_offset = frame.block_base`。`None` は呼び出し側（static 初期化子を持つ関数だけを呼ぶ）の不整合なので `expect` で panic する |
| `global_heap_size` | `layout.total_size()` を返す |
| `get_var_info` | `static_var_global_offsets.get(..)` を `layout.static_var_offset(..)` に。条件の修正は [02](02-static-var-in-nested-block.md) |
| getter 追加 | `next_var_offset()`（下記 debug_assert 用） |

`statement.rs` の `generate_scope` ② は、`func_scope_var_count`・`total_var_count` の計算をやめ、`enter_function_for_static_init(func_idx, &func.block.scope)` を呼ぶ。

`alloc_runtime/mod.rs:28` の `global_heap_size` の doc（「グローバル変数 + static 変数の合計サイズ」）を「グローバル領域（グローバル変数・static 変数・初期化文が使う変数）の合計サイズ」に直す。

### 確認用の debug_assert

数え上げとコード生成の不一致（将来ブロックを持つ構文を足したとき等）を debug テストで検出するため、`next_var_offset` が予約した領域の末尾を超えないことを `debug_assert!` で確認する。危険なのは数え不足（隣の領域に食い込む）なので `<=` で確認する。

- `generate_scope` の ①③ の後: グローバルの `next_var_offset() <= root_init_block_range().end`
- ② の各関数の後: `static_ctx.next_var_offset() <= static_init_frame(func_idx).end`
- `generate_function_definition` の後: `local_ctx.next_var_offset() <= total_var_count`（`block_var_count` を共有するので同じ確認を足す）

### バグのコメント

AGENTS.md に従い、`GlobalLayout::compute`（領域を予約する箇所）と `new_with_options`・`enter_function_for_static_init`（オフセットの初期値）に、原因（ルート・static 初期化子の変数が予約の無いオフセット 0 / `func_scope_var_count` から積まれ、グローバル変数等と重なっていた）と解決策（グローバル領域の末尾に予約）を書く。

## テスト

### large テスト（`resources/tests/passes/scope/`、`success_io`）

README の再現を元に、run と WS で同じ出力になることを確認する。名前は既存の `block_var_no_collision_001` に揃え、`scope_` を付けない（manifest 上は `test_scope_<name>`）。

| ファイル | 内容 | 期待 stdout |
|---|---|---|
| `root_block_var_no_collision_001` | A1 + A2 + A4（ルートの `{}`・`if`・namespace 内のブロック変数、配列を含む）。ルートのブロック内で関数を呼んだ後にブロック変数を読む。グローバル変数・グローバル配列の値を出力 | グローバルが上書きされず、ブロック変数も正しい |
| `root_static_init_block_var_001` | A3（ルートの static 初期化子・`let` 初期化子のブロック）。`g, h, k` を出力 | `1 9 4` 相当 |
| `func_static_init_block_var_001` | B1 + B2 + C3。関数内 static 初期化子のブロック変数・非 static 変数への書き込み。複数関数の static 初期化子それぞれにブロック変数を置く。グローバルはルートの `static:` にする（ルートの `let:` は関数内 static 初期化子の後に初期化されるため上書きが観測できない） | static 値・グローバル値が正しい |
| `func_static_init_block_var_alloc_001` | B1 の FSBA 表破壊。`__alloc` した領域に書いて読む。`std_ext: ["alloc"]`、`exclude_targets: [whitespace]`（既存の alloc テストと同じ。既定の `WsCompileOptions` は alloc 無効） | alloc 領域の値が正しい |
| `template_static_init_block_var_001` | B3（テンプレート関数の 2 インスタンスの static 初期化子にブロック） | `1 2 3 4` 相当 |

`test-manifest.yaml` に追加する（既存の `test_scope_block_var_no_collision_001` と同じ形式）。アロケータは既定の FSBA で確認する。Bump では `LOCAL_HEAP_END` の初期値が変わるだけなので、別のテストは置かない。

### unit テスト（`src/compiler_ws/global_layout_tests.rs`）

`GlobalLayout::compute` は意味解析の結果（`Scope`）の構造と数え上げ（`block_var_count`）に依存し、自明でないので unit テストを置く。`Scope` はソース文字列を `parse_to_tokens` → `parse_to_tree` → `analyze` して作る（`src/interpreter/exec_tests.rs:26` と同じ方法）。

- グローバル 2 個・static 1 個・ルートのブロック変数 3 個 → `root_init_block_range == 3..6`、`total_size == 6`
- 2 関数の static 初期化子にそれぞれブロック変数 → 領域が重ならず、各領域が `variable_count + ブロック変数数`、`total_size` が合計になる
- 未使用関数の static 初期化子にブロック変数 → 領域が予約される（未使用でも数える方針の固定）
- 初期化文にブロックが無く、static 初期化子も無い → `total_size == V + S`（既存と同じ値。既存の出力が変わらないこと）
