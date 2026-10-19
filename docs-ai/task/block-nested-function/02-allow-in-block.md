# B: ブロックスコープ内の関数宣言を許可

ブロックスコープでも、関数スコープと同じように関数を宣言できるようにする。意味解析の関数表を、式の変換を含むすべての経路で共有する。あわせて、ブロック越しに非 static 変数を参照できてしまう関数境界の判定を直す。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

前提: [01-forbid-in-block.md](01-forbid-in-block.md)（A）が完了していること

## 設計・方針

### 原因の整理

ブロック内の関数宣言が正しく動かない原因は 2 つある。

| # | 原因 | 箇所 |
|---|---|---|
| 1 | ブロックの解析に一時的な空の関数表を渡している。ブロック内の関数はそこに登録されて捨てられ、インデックスは本来の関数表の別の関数を指す | `analyze_block_for_expression`（`mod.rs:60`）。呼び出し元は式の `If` / `Block` / ブロックエイリアス展開（`expression.rs`）と、文の `While` / `For` の cond・step・body（`statement.rs`） |
| 2 | 型検査に使う戻り値型の一覧（`func_return_types: &[ValueType]`）は、各スコープの解析開始時に作る複製である。子のブロックで追加した関数の戻り値型は親の一覧にない。`infer_type` は見つからない関数を `Void` として扱う | `analyze_internal_with_parent` の `effective_func_return_types`、`AnalyzeContext::inherited_func_return_types`。for-init の再現（README）はこちらが原因 |

1 だけを直すと、2 によってブロックの値（`{ func: g() {...} g(); }` の値）や for 本体での呼び出しの型が `Void` になる。両方を直す。

式の変換（`convert_to_exec_expression_with_resolver`）は現在 `AnalyzeContext` を受け取らない。関数表は `&mut` で持ち回る必要があるので、式の変換にも引数を追加する。

### Step 1: FunctionTable の導入と引き回し（挙動は変えない）

#### FunctionTable（`src/semantic_analyzer/function_table.rs` 新規）

`global_functions: Vec<Function>`・`global_function_names: Vec<String>`・戻り値型の一覧を 1 つの構造体にまとめる。戻り値型は関数宣言のスキャン時に確定するので、登録と同時に一覧へ追加すれば、複製が古くなることはない。

```rust
/// 意味解析中のグローバル関数表
///
/// ネスト関数・ブロック内の関数を含む全関数を、宣言を見つけた順に登録する。
/// 登録順のインデックス（グローバル関数インデックス）が `IdentifierRef::local_index` などから参照される。
pub(super) struct FunctionTable {
    functions: Vec<Function>,
    /// マングル済みの関数名（`functions` と同じ順）。同名の関数が複数あってもよい
    names: Vec<String>,
    /// `functions[i].return_type` と同じ値。`infer_type` に `&[ValueType]` で渡すため別に持つ
    return_types: Vec<ValueType>,
}

impl FunctionTable {
    pub(super) fn new() -> Self;
    /// 関数宣言のスキャン（`scan_function_declarations`）: 関数を登録し、グローバル関数インデックスを返す。本体は空のブロック
    pub(super) fn declare(&mut self, mangled_name: String, return_type: ValueType) -> usize;
    /// パス 2: 本体の解析結果を設定する。戻り値型は declare 時の値のまま（変えない）
    pub(super) fn define(&mut self, index: usize, arg_indices: Vec<usize>, block: Block);
    pub(super) fn return_type(&self, index: usize) -> &ValueType;
    pub(super) fn return_types(&self) -> &[ValueType];
    /// `ScopeBuilder::build` に渡すため、関数と関数名に分解する
    pub(super) fn into_parts(self) -> (Vec<Function>, Vec<String>);
}
```

- `scan_function_declarations`（`mod.rs:558-660`）の `ctx.global_functions.push(Function { ... 空の Scope ... })` と `global_function_names.push` を `declare` に置き換える。空の本体は `Function::dummy()` と同じ形だが、`is_unused` が `false` で戻り値型が異なるので、`declare` の中で組み立てる
- `statement.rs:355` の `ctx.global_functions[global_idx] = Function { ... }` を `define` に置き換える。戻り値型を `identifier_map` から取り直している処理（`statement.rs:345-353`）は不要になる
- `statement.rs:389` の `ctx.global_functions[idx].return_type` は `return_type(idx)` にする
- `return_types` に書き込むのは `declare` だけにする。`define` は `functions[index].return_type` を変えないので、2 つの一覧は常に一致する（`define` に `debug_assert_eq!` を置いてもよい）
- `mod.rs` 冒頭の「モジュール構成」コメントに `function_table` を追加する
- `FunctionTable` は関数表の登録・参照だけを持つ。戻り値型の決定（`has_return_statement` などの検査）は `scan_function_declarations` に残す

#### AnalyzeContext の変更（`context.rs`）

```rust
pub(super) struct AnalyzeContext<'a> {
    /// グローバル関数表（全スコープの解析で共有する）
    pub functions: &'a mut FunctionTable,
    /// 現在解析中の関数のグローバルインデックス（None = ルートスコープ・ブロックスコープ）
    pub func_global_index: Option<usize>,
}
```

- `global_functions` / `global_function_names` を `functions` にまとめる
- `inherited_func_return_types` は削除する。型検査は常に `ctx.functions.return_types()` を使う
- `new_root` / `new_function` に加えて、ブロック用の `new_block(functions)`（`func_global_index: None`）を用意する
- `func_global_index` をブロックで `None` にする現在の挙動は変えない。ブロック内の `return:` の void 検査が省略されている点（`statement.rs:386-394`）や、for-init で `None` にしている点（`statement.rs:478-480`）も今回は変えない

#### 戻り値型一覧の引数を関数表に置き換える

`func_return_types: &[ValueType]`（expression.rs 32 箇所、statement.rs 25 箇所、mod.rs 7 箇所）を、関数表から取得するように置き換える。

| 関数 | 現在の引数 | 変更後 |
|---|---|---|
| `convert_to_exec_expression_with_resolver`（`expression.rs:81`） | `(expr, resolver, func_return_types)` | `(expr, resolver, functions: &mut FunctionTable)` |
| `analyze_block_for_expression`（`mod.rs:60`） | `(stmts, resolver, func_return_types)` | `(stmts, resolver, functions: &mut FunctionTable)`。中で `AnalyzeContext::new_block(functions)` を作る |
| `convert_to_exec_statements`（`statement.rs:36`） | `(..., effective_func_return_types, ctx)` | `(..., ctx)` |
| `require_int_type`（`expression.rs:23`）・`infer_type`・`infer_block_type` | `&[ValueType]` | 変えない。呼び出し側で `functions.return_types()` を渡す |

- 式の変換は `AnalyzeContext` 全体ではなく `&mut FunctionTable` だけを受け取る。式の中で必要なのは関数表だけで、`func_global_index` は使わない
- 呼び出し側の書き方は、変換してから検査する順になる（`let e = convert(expr, resolver, functions)?; require_int_type(&e, functions.return_types())?;`）。可変借用と共有借用が同時に生きる箇所はない想定。衝突した場合は `return_types()` の結果を一時変数に取らずに、その場で呼ぶ
- `analyze_internal_with_parent` の `effective_func_return_types` の計算（`mod.rs:186-196`）は削除する
- `for` の init 用に `AnalyzeContext` を手で組み立てている箇所（`statement.rs:481-486`）は、`func_global_index: None` を保ったまま `functions: &mut *ctx.functions` に置き換える
- `analyze`（`mod.rs:291`）は `FunctionTable::new()` を作り、最後に `into_parts()` で `scope.build` に渡す

#### Step 1 で挙動が変わらないこと

A の検査が有効な間は、ブロックスコープに関数宣言は現れない。したがって、ブロックの解析で関数表に追加されるものはなく、`return_types()` は以前の複製と同じ内容になる。以前の複製との違いは、子スコープで追加された関数の戻り値型が見えることだけで、その状況は A で起きなくなっている。

Step 1 は単独のコミットにし、`cargo test` が全件通ることを確認してから Step 2 に進む。挙動を変えないので VERSION.md には書かない（Step 2 の内容だけを書く）。

### Step 2: 禁止の解除と関数境界の判定修正

#### 禁止の解除

- A で追加した `reject_function_declarations_in_block` とその呼び出しを削除する
- `analyze_block_for_expression` の doc コメントを「親の関数表を共有するため、ブロック内で宣言した関数も全体の関数表に登録される」に書き換える
- `analyze_block_for_expression` と `ScopeType::Block` の説明に、元のバグ（一時的な空の関数表で誤解決していた）と解決策（関数表の共有）をコメントで残す

#### 関数境界の判定修正（nested-block-function-and-static #3）

`resolve_variable_exact`（`scope.rs:340`）と `resolve_variable_with_type_exact`（`scope.rs:398`）は、「最も内側の関数スコープより外側にある、**関数スコープ**」で見つけた非 static 変数だけを読み飛ばしている。

```rust
let crossed_function_boundary =
    if let Some(first_func_depth) = first_function_scope_depth {
        depth > first_func_depth && scope_info.is_function_scope
    } else { false };
```

これまでネスト関数は関数スコープの直下にしか置けなかったので、関数スコープ同士の間にブロックスコープが挟まることはなかった。ブロック内の関数を許すと、`[root, f, block, g]` のようにブロックスコープが挟まる。`g` から `block` の非 static 変数を探すと、`block` は関数スコープではないので境界を越えたと判定されず、参照できてしまう。`g` は独立したフレームで実行されるので、このアドレスは誤りになる（WS では `get_var_info` の fallback でグローバル変数と重なる可能性がある）。

修正: 最も内側の関数スコープより外側なら、スコープ種別によらず境界を越えたとする。

```rust
// バグ: 関数スコープの段だけを境界とみなしていたため、関数とその外側の関数の間にあるブロックスコープの
// 非 static 変数を、内側の関数から参照できてしまっていた。
// 解決策: 最も内側の関数スコープより外側の段は、スコープ種別によらず境界の外とする。
let crossed_function_boundary =
    first_function_scope_depth.is_some_and(|first_func_depth| depth > first_func_depth);
```

- 2 つの関数に同じ判定があるので両方直す
- 外側にある static 変数・グローバル変数（ルートの変数は暗黙に static）は、これまでどおり参照できる。`owning_func_index` は見つけたスコープの `func_global_index` で決まる。関数スコープ直下の static であれば正しく引ける
- ブロックスコープで宣言した static（`func_global_index` が `None` のスコープ）を内側の関数から参照する場合は、`owning_func_index` が `None` になり正しく引けない。ブロック内の static 宣言自体が既に壊れている（nested-block-function-and-static #2）ので、本タスクでは扱わない

#### 名前空間内の関数での登録名の不一致

名前空間内の関数の本体で関数を宣言すると、現在でも panic する（A の「既知の問題」）。

```
namespace: N { func: f() { func: h() { return: 1; } return: h(); } }
→ panic: internal error: function 'h' should be pre-registered in pass 1a (mangled: 'N$h')
```

`analyze_internal_with_parent` は `scan_function_declarations(statements, "", ...)` で、本体のスコープの中だけのプレフィックス（`""`）を使って `h` を登録する。一方、パス 2 の `FunctionDeclaration`（`statement.rs:305-314`）は、親から継承した `resolver.current_ns_prefix()`（`N$`）を付けた `N$h` で探す。ブロック内の関数も同じ経路を通るので、B で禁止を外すとブロックでも再現する。

修正: 関数宣言のスキャンを、親のリゾルバの名前空間プレフィックスから始める。

```rust
// バグ: 関数宣言のスキャンは "" から、パス 2 の参照は親から継承したプレフィックスから名前を作っていたため、
// 名前空間内の関数の本体（とその中のブロック）で宣言した関数が見つからず panic していた。
// 解決策: スキャンも親の名前空間プレフィックスから始め、パス 2 と同じ名前で登録する。
let ns_prefix = parent_resolver.map(|p| p.current_ns_prefix()).unwrap_or_default();
scan_function_declarations(statements, &ns_prefix, &mut scope, ctx)?;
```

- 登録名は `N$h` になる。本体からの呼び出し `h()` は `ns_candidate_names` が `N$h` → `h` の順に探すので解決できる
- 変数の登録（`scan_variable_declarations`）は `""` のままにする。本体の直下の変数は `ns_candidate_names` の最後の候補（修飾なしの名前）で解決できる。ただし、名前空間内の関数の本体にさらに `namespace: M` を書くと、変数は `M$v` で登録され、参照側は `N$M$v` → `N$v` → `v` を探すので見つからない（現在でも `undefined variable` になる）。変数の登録名を揃えると `variable_indices` のキーが変わり、引数の参照（`built_scope.variable_indices.get(arg_name)`）や static 初期化の経路に影響するため、本タスクでは扱わず「対象外・懸念」に記録する
- 同じ修正で、名前空間内の関数の本体でのテンプレートのインスタンス化（`alias:`）と、本体の `namespace: M { func: k() ... }`（登録名 `N$M$k`）も panic しなくなる
- ルートのスコープは `parent_resolver` が `None` なので `""` から始まり、現在と変わらない

#### final・配列サイズの参照で関数境界を見ていない

`is_final_variable`（`scope.rs:451`）と `get_array_size`（`scope.rs:470`）は、スコープを内側から順に探して最初に見つかった同名の変数を返す。関数境界で非 static 変数を読み飛ばさないので、`resolve_variable_exact` が解決した変数と別の変数の情報を返すことがある。現在でも再現する。

```
let: x(5);
func: f() { final: x(0); func: g() { x = 7; return: x; } return: g(); }
→ cannot assign to final variable 'x'（g の x はグローバル変数なので代入できるはず）
```

ブロック内の関数を許すと、`{ final: x(0); func: g() { x = 1; } }` や、ブロックの配列と同名のグローバル変数の添字参照でも起きるようになる。

修正: 境界を考慮した探索を 1 つの private 関数にまとめ、3 つの参照で共有する。

```rust
/// 関数境界を考慮した変数探索で見つかった変数
struct FoundVariable<'s, 'a> {
    /// 現在のスコープからの段数
    depth: usize,
    /// 見つけたスコープ内のスロットインデックス
    local_index: usize,
    /// 最も内側の関数スコープより外側で見つけたか
    crossed_function_boundary: bool,
    scope: &'s ScopeInfo<'a>,
    var: &'s Variable,
}

/// 変数探索の結果
enum VariableLookup<'s, 'a> {
    Found(FoundVariable<'s, 'a>),
    NotFound,
    /// 参照できない変数を見つけた（ブロックの static を内側の関数から参照した。次節）
    Forbidden(String),
}

/// 名前を完全一致で探す（関数境界を考慮）
fn find_variable_exact(&self, name: &str) -> VariableLookup<'_, 'a>;
/// 名前空間の候補（`ns_candidate_names`）と import（`resolve_imported_name`）を含めて探す
fn lookup_variable(&self, name: &str) -> VariableLookup<'_, 'a>;
```

- `lookup_variable` の候補順は、現在の `resolve_variable` / `resolve_variable_with_type` と同じにする（候補 → import の順）
- `resolve_variable` / `resolve_variable_with_type` / `is_final_variable` / `get_array_size` はすべて `lookup_variable` の上に作る。`IdentifierRef` は `depth`・`local_index`・`crossed_function_boundary`（`owning_func_index` の決定に使う）から組み立てる。境界判定は `find_variable_exact` の 1 箇所だけになる
- 現在の `is_final_variable` / `get_array_size` は import のフォールバックを持たないので、この変更で 4 つの参照の対象が一致する
- 境界判定の修正（前節）はこの関数に入れる
- `array_size` の誤りは、現在どのバックエンドも `ExecExpression::ArrayAccess` の `array_size` を使っていないので、実行結果には現れない。large テストでは確認せず、`semantic_analyzer/tests.rs` の単体テストで解決結果を確認する

#### ブロックの static を内側の関数から参照したときのエラー

境界判定を直したあとも、ブロックスコープで宣言した static をそのブロック内の関数から参照すると、`crossed_function_boundary` は真、`owning_func_index` は `None`（ブロックのスコープの `func_global_index` は `None`）になる。WS コンパイラの `get_var_info` は、このとき fallback のアドレス（フレーム相対、または `Global(local_index)`）を使うので、エラーにならずに誤ったコードになる。

ブロック内の static 宣言は nested-block-function-and-static #2 で扱うが、それまでの間、この組み合わせは意味解析エラーにする。

- `find_variable_exact` で `crossed_function_boundary && var.is_static` かつ、見つけたスコープが関数スコープでない（`owning_func_index` を決められない）場合
- この条件を満たしたら、探索をそこで打ち切り `VariableLookup::Forbidden` を返す。非 static の場合のように `continue` して外側を探してはいけない（同名のグローバル変数に解決され、再び誤ったコードになる）
- 公開側の `resolve_variable` / `resolve_variable_with_type` の戻り値も、`Option` から `Forbidden` を区別できる形（`Result<Option<..>, String>` など）に変える。呼び出し元（`expression.rs` の `&x`・`&a[i]`・struct の型注釈・変数参照・`a[i]`、`statement.rs:52`）はすべて、`Forbidden` を `undefined variable` ではなく次のエラーにする
  - `semantic error: static variable '{name}' declared in a block scope cannot be referenced from a nested function`
- `is_final_variable` / `get_array_size` は、`Forbidden` を「見つからない」と同じに扱ってよい（同じ式の変数参照でエラーになるため）
- #2 で static の配置を直したら、このエラーを外す（#2 のドキュメントにも記載する）

#### 言語仕様（`docs/spec.md`）

A で追加した「ブロックスコープ内の関数定義はコンパイルエラー」を、次の内容に置き換える。

- 関数はブロックスコープ（`{}`、`if` / `while` / `for` / `repeat` の本体など）の中でも定義できる
- ブロック内で定義した関数は、そのブロックとその子スコープからだけ呼び出せる。ホイスティングはそのブロックの中で行われる
- 外側の関数スコープ・ブロックスコープの非 static 変数は参照できない（関数スコープの規則と同じ）。static 変数・グローバル変数は参照できる
- 同じ名前の関数が外側にある場合は、内側（ブロック内）の関数が優先される
- ブロック内で宣言した static 変数は、そのブロック内の関数から参照できない（当面の制限）
- ブロックエイリアスの本体に関数定義を書いた場合、展開した箇所ごとに別の関数が作られる（その関数の static 変数も展開ごとに独立する。テンプレート関数のインスタンスと同じ）

「スコープ」節の「関数スコープは…static でない親の変数にアクセスすることはできない」の記述は、ブロックスコープの変数も含むことがわかるように補う。

#### テスト

A の `fails/compile/func_in_*` 9 件は削除し、manifest からも外す。代わりに次を追加する。

success_io（`resources/tests/passes/scope/`、全ターゲット）:

| ファイル | 内容・期待 |
|---|---|
| `func_in_block_expr_001` | 指摘の再現（`__putc(65); { func: g(){ __putc(66); return: 5; } g(); };`）が `AB` を出力して終了する |
| `func_in_block_value_001` | `func: h(){return:9;}` を先に置き、`__puti({ func: g(){return:1;} g(); })` が `1`。ブロックの値の型推論（原因 2）の確認 |
| `func_in_if_while_001` | `if:` / `else:` / `while:` の本体で定義した関数を呼ぶ |
| `func_in_for_init_001` | for-init で定義した関数を body から呼び、戻り値を使う（README の void エラーの再現） |
| `func_in_block_access_001` | ブロック内の関数から、グローバル変数・外側の関数の関数スコープ直下の static を読み書きする。自分自身の再帰呼び出しと外側の関数の呼び出しも行う |
| `func_in_block_shadowing_001` | ルートの `g` と同名の `g` をブロック内で定義すると、ブロック内では内側、外では外側が呼ばれる。兄弟ブロックの同名関数がそれぞれ独立する |
| `func_in_block_alias_001` | 関数と static を含むブロックエイリアスを 2 箇所で展開し、それぞれの static が独立している |
| `func_in_root_block_001` | ルートの式文のブロック内で定義した関数を、同じブロック内で呼ぶ |
| `func_in_block_hoist_001` | ブロック内で、宣言より前の位置から関数を呼ぶ |
| `func_in_loop_bodies_001` | `repeat:` の本体、`else:` の本体、`for:` の cond・step ブロックで定義した関数を呼ぶ |
| `func_in_block_namespace_001` | ブロック内の `namespace:` の関数を `NS$g()` で呼ぶ |
| `func_in_block_template_001` | ブロック内でテンプレート関数をインスタンス化して呼ぶ |
| `func_nested_in_namespace_001` | `namespace: N { func: f() { func: h() {...} return: h(); } }`、`f` の中のブロックで定義した関数、`f` の本体でのテンプレートのインスタンス化、`f` の本体の `namespace: M { func: k() ... }` を `M$k()` で呼ぶ（panic の修正） |
| `final_shadow_nested_func_001` | `let: x(5); func: f() { final: x(0); func: g() { x = 7; return: x; } ... }` が `7` を返す（final 判定の修正）。ブロック内の関数版も含める |

compile_error（`resources/tests/fails/compile/`）:

| ファイル | 内容・期待（`contains`） |
|---|---|
| `func_in_block_call_outside_001` | ブロック内の関数をブロックの外から呼ぶ → `undefined function` |
| `func_in_block_nonstatic_access_001` | 関数 `f` 内のブロックの変数を、そのブロック内の関数から参照する → `undefined variable` |
| `func_in_root_block_nonstatic_access_001` | ルートのブロックの変数を、そのブロック内の関数から参照する → `undefined variable`（#3 の再現） |
| `func_in_for_init_counter_access_001` | for-init で宣言したループ変数を、for-init で定義した関数から参照する → `undefined variable` |
| `func_in_block_static_access_001` | ブロックの static を、そのブロック内の関数から参照する → `cannot be referenced from a nested function`。同名のグローバル変数を置いた版も含め、そちらに解決されずにエラーになることを確認する |

`undefined variable` を期待する 3 件は、外側に同名のグローバル変数を置かない（読み飛ばしたあと外側の探索に進むため、同名があるとそちらに解決されてエラーにならない）。

単体テスト（`semantic_analyzer/tests.rs`）: 関数内のブロックに配列 `a[3]`、同名のグローバル配列 `a[5]` を置き、ブロック内の関数から `a[i]` を参照したとき、解決される `IdentifierRef` がグローバル側で、`array_size` が 5 になること。

既存の関数境界のテスト（`scope_static_error_001`、`static_in_block_nested_func_001` など）が通り続けることも確認する。

#### VERSION.md

3.0.0 のリリース後に作る WIP セクション（MINOR を含むので `3.1.0 (WIP)`）に記載する。3.0.0 の WIP には入れない。B が 3.0.0 のリリース前に用意できた場合も、リリースを待ってから入れる。

- MINOR: `Language: Functions can be declared in block scopes (`{}`, `if` / `while` / `for` / `repeat` bodies, block aliases). They are visible only inside the block and cannot read non-static variables of enclosing blocks (block `static` variables are not accessible yet)`
- PATCH: `Semantic analyzer: Fix an internal panic when a function is declared inside a function in a namespace`
- PATCH: `Semantic analyzer: A nested function no longer takes the final flag from a same-named non-static variable of its enclosing function`

境界判定の修正（#3）は、A のあとはブロック内に関数を書けないので 3.0.0 では到達できない。MINOR の項目に含め、PATCH には書かない。

### 対象外・懸念

- ブロック内の static 宣言（nested-block-function-and-static #2）は扱わない。ブロック内の関数からブロックの static を参照する組み合わせは、当面エラーにし（前述）、#2 で許可する
- 名前空間内の関数の本体に `namespace: M { let: v; }` を書くと、変数 `M$v` を参照できない（登録は `M$v`、参照は `N$M$v` / `N$v` / `v` を探す）。現在でも起きる。変数の登録名を親の名前空間プレフィックスから始める修正は、引数・static 初期化の経路への影響を確かめる必要があるので別タスクにする
- 同じく名前空間内の関数の本体での `import:` は、`collect_import_bundle` / `collect_namespace_info_recursive` が `""` から名前空間キーを作るため、`current_ns_key()`（`N`）と一致せず効かない。現在でも起きる問題で、本タスクでは扱わない
- `__main` の検出（`ScopeBuilder::build` の `function_names.iter().position(|name| name == "__main")`）は、ルート以外で定義した `__main` も対象になる。ルートに `__main` がなく、ネスト関数やブロック内の関数に `__main` があると、それがエントリになる。ネスト関数では現状でも起きることで、本タスクでは変えない（名前空間内の関数のネスト関数は、名前空間の修正で `N$__main` として登録されるので対象外になる）。必要なら別タスクで「ルートスコープの `__main` に限る」ようにする
- WS コンパイラ・インタプリタは、フラット化された関数表とグローバル関数インデックスで関数を扱っており、ブロック内の関数も同じ形で渡る。変更は不要の想定。large テストを全ターゲットで流して確かめる。問題が出た場合は本ドキュメントに追記する

## タスクステップ・チェックリスト

1. [ ] Step 1: FunctionTable の導入と引き回し（挙動は変えない）
   1. [ ] `function_table.rs` を追加し、`scan_function_declarations` / `FunctionDeclaration` の解析 / `analyze` を移行
   2. [ ] `AnalyzeContext` から `global_functions` / `global_function_names` / `inherited_func_return_types` を削除し `functions` に統一
   3. [ ] 式の変換・`analyze_block_for_expression`・`convert_to_exec_statements` の `func_return_types` 引数を `&mut FunctionTable` に置き換え
   4. [ ] `cargo test` 全件が通ることを確認
2. [ ] Step 2: 禁止の解除と関数境界の判定修正
   1. [ ] `reject_function_declarations_in_block` を削除し、コメントを更新
   2. [ ] 関数宣言のスキャンを親の名前空間プレフィックスから始める（バグコメント付き）
   3. [ ] `find_variable_exact` / `lookup_variable` を追加し、境界判定を修正（バグコメント付き）。`resolve_variable` / `resolve_variable_with_type` / `is_final_variable` / `get_array_size` をその上に作り直す
   4. [ ] ブロックの static を内側の関数から参照したときのエラーを追加（`VariableLookup::Forbidden`、呼び出し元 6 箇所の対応）
   5. [ ] A のテスト 9 件を削除し、success_io 14 件・compile_error 5 件・単体テスト 1 件を追加
   6. [ ] `docs/spec.md` を更新
   7. [ ] VERSION.md（`3.1.0 (WIP)`）に追記
   8. [ ] `cargo test` 全件を確認。docs-ai の 02・nested-block-function-and-static の状態を更新
