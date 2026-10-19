# A: ブロックスコープ内の関数宣言を禁止

意味解析で、ブロックスコープ（`ScopeType::Block`）に現れた関数宣言をエラーにする。誤ったコードを生成せずに、コンパイルエラーで止める。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

### 許可する場所・禁止する場所

| 場所 | スコープ種別 | A の扱い |
|---|---|---|
| グローバルスコープ直下 | `Root` | 許可（現状どおり） |
| 関数本体の直下（ネスト関数） | `Function` | 許可（現状どおり。`func_nested_001` など） |
| 上の 2 つの直下にある `namespace:` の中 | 親と同じ | 許可（現状どおり） |
| `{ ... }`（ルートの式文を含む）、`if` / `elsif` / `else` / `while` の本体 | `Block`（`analyze_block_for_expression`） | **エラー** |
| `for` の init / cond / step / body | `Block` | **エラー**（init は関数表を共有しているが、前述のとおり型検査が壊れている） |
| `repeat` の本体 | `Block`（パーサで `for` に脱糖される） | **エラー** |
| ブロックエイリアスの本体 | 展開先の `Block` | **エラー**（定義だけでは解析されない。展開したときに検出する） |
| ブロックスコープ内の `alias:` によるテンプレートのインスタンス化 | `Block` | **エラー**（プレパスで `FunctionDeclaration` に展開されたあとで検出する） |
| ブロックスコープ内の `namespace:` の中 | `Block` | **エラー**（`scan_function_declarations` が名前空間の中まで登録するため、同じく再帰的に調べる） |

`constexpr:` のブロックは別の評価器で処理され、`func:` は既に `unsupported statement in constexpr block` で拒否されているので対象外。

テンプレート関数の「定義」（`TemplateFunctionDefinition`）だけがブロック内にあり、インスタンス化されない場合は、プレパスで取り除かれて関数が作られないので誤コードにならない。A ではエラーにしない（B で許可するので、A で禁止を広げる理由がない）。

### 検出箇所

`analyze_internal_with_parent`（`src/semantic_analyzer/mod.rs:97`）で、テンプレート展開（`expand_template_instantiations`）の直後、パス 0 より前に検査する。ブロックスコープの解析はすべてこの関数を通る（`analyze_block_for_expression` 経由と、for-init の直接呼び出し）ので、ここ 1 箇所で全経路を押さえられる。

```rust
// バグ: ブロックスコープの関数宣言は一時的な空の関数表に登録され、解析後に捨てられていた。
// 呼び出しのインデックスが本来の関数表の別の関数（多くは __main）を指し、誤ったコードを生成していた。
// 解決策: ブロックスコープでの関数宣言を意味解析エラーにする。
if matches!(scope_type, ScopeType::Block) {
    let ns_prefix = parent_resolver.map(|p| p.current_ns_prefix()).unwrap_or_default();
    reject_function_declarations_in_block(statements, &ns_prefix)?;
}
```

```rust
/// ブロックスコープ内の関数宣言をエラーにする（名前空間の中も調べる）
///
/// `ns_prefix` はエラーメッセージに出す関数名のマングル用（親の名前空間プレフィックスから始める）
fn reject_function_declarations_in_block(
    statements: &[LocatedStatement],
    ns_prefix: &str,
) -> Result<(), Vec<CodeParseError>>
```

- `Statement::FunctionDeclaration(name, ..)` を見つけたら、`located_stat.location.start` の位置で次のエラーを返す
  - `semantic error: function '{ns_prefix}{name}' cannot be declared in a block scope (only at global or function scope)`
- `Statement::NamespaceDeclaration(ns_name, body)` は `scan_function_declarations` と同じ規則でプレフィックスを作って再帰する
- 文の中の式（`if` の本体など）には入らない。子のブロックは、自分の `analyze_internal_with_parent` で検査される
- 置き場所は `mod.rs` の `scan_function_declarations` の近く。B で削除する前提の小さな関数なので、新しいモジュールは作らない

### コメントの修正

`analyze_block_for_expression` の doc コメント「ブロック式の中では新しい関数宣言は許可されないため、グローバル関数リストへの参照として一時的な空 Vec を使用する」を、実装と合うように書き換える。

> ブロックスコープでの関数宣言は `analyze_internal_with_parent` でエラーにしているため、この関数表には何も登録されない。一時的な空 Vec を渡しているのはそのため（B で親の関数表を共有する予定）。

### spec（`docs/spec.md`「関数定義」）

関数定義の節に、定義できる場所を追記する。

- 関数はグローバルスコープ、関数本体の直下、およびそれらの直下の名前空間の中で定義できる
- ブロックスコープ（`{}`、`if` / `while` / `for` / `repeat` の本体、ブロックエイリアスの本体など）の中で関数を定義するとコンパイルエラーになる。テンプレート関数のインスタンス化（`alias:`）も同じ

「static 変数」節の例（`fn1` の中の `fn2` など）は関数本体直下での定義なので変更しない。

### テスト（large、`resources/tests/fails/compile/`、`type: compile_error`）

`.check.json` は `"contains": ["cannot be declared in a block scope"]` とする。各 `.ns` には `__main` を置き、エラーの原因がブロック内の関数宣言だけになるようにする。

| ファイル | 内容 |
|---|---|
| `func_in_block_expr_001` | `__main` 内の `{ func: g() { ... } g(); };`（指摘の再現そのもの） |
| `func_in_root_block_001` | ルートの式文 `{ func: g() { ... } };` |
| `func_in_if_001` | `if:` の本体（`else:` 側でも同じ経路なので 1 件でよい） |
| `func_in_while_001` | `while:` の本体（文の `While` は式の `If` と別の呼び出し箇所） |
| `func_in_for_init_001` | `for:` の init ブロック（`analyze_block_for_expression` を通らない経路） |
| `func_in_block_alias_001` | 関数宣言を含むブロックエイリアスを `__main` 内で展開 |
| `func_in_block_namespace_001` | ブロック内の `namespace:` の中の関数宣言 |
| `func_in_block_template_001` | ブロック内でテンプレート関数を `alias:` でインスタンス化 |
| `func_in_namespaced_func_block_001` | `namespace: N { func: f() { { func: g() {...} g(); }; } }`。現状は内部エラーの panic（下記）になる経路で、A ではコンパイルエラーになる。メッセージに `N$g` が出ることも確認する |

`test-manifest.yaml` の該当セクション（`test_scope_scope_nested_func_child_access_error_001` の近く）に `test_compile_error_func_in_*` として登録する。

既存の large テストに、ブロック内で関数を宣言しているものはない（`resources/` と `docs/` を grep して確認済み）。実装後は `cargo test` 全体を流し、回帰がないことを確かめる。

### VERSION.md

3.0.0（WIP）の PATCH に追加する。これまで受理されていたプログラムがエラーになるが、それらは正しく動いていなかったので互換性の破壊には当たらない。

- `Semantic analyzer: Reject function declarations in block scopes (`{}`, `if` / `while` / `for` / `repeat` bodies, block aliases) as a compile error. They were previously accepted and calls resolved to an unrelated function (inside a namespaced function, they caused an internal panic)`

### 既知の問題（A では直さない）

名前空間内の関数の本体で、直下にネスト関数を宣言すると panic する（`namespace: N { func: f() { func: h() { return: 1; } return: h(); } }` → `internal error: function 'h' should be pre-registered in pass 1a (mangled: 'N$h')`）。関数宣言のスキャンは本体のスコープの中だけのプレフィックス（`""`）で登録するのに、変換時の参照は親から継承したプレフィックス（`N$`）を付けるためである。ブロック内の関数でも同じ経路を通るので、本体でのテンプレートのインスタンス化（`alias:`）や、本体の `namespace: M { func: k() ... }` でも同じく panic する。B で一緒に直す（[02-allow-in-block.md](02-allow-in-block.md)「名前空間内の関数での登録名の不一致」）。A ではブロックの場合だけが検査で先にエラーになる。

### ドキュメントの更新（docs-ai）

- `code-review-2026-10/02-block-nested-function-misresolution.md`: 状態を「A 完了、B は [block-nested-function](../block-nested-function/README.md)」にする
- `nested-block-function-and-static.md`: #1 は A で「エラー」になったことを記録する。#3 はブロック内に関数を書けなくなったので、A の時点では再現しない（B で再び到達可能になるので B で直す）

## タスクステップ・チェックリスト

1. [ ] `reject_function_declarations_in_block` を追加し、`analyze_internal_with_parent` から呼ぶ（バグコメントを付ける）
2. [ ] `analyze_block_for_expression` の doc コメントを修正
3. [ ] large テスト 9 件を追加し、manifest に登録
4. [ ] `docs/spec.md` の「関数定義」に定義可能な場所を追記
5. [ ] VERSION.md（3.0.0 WIP の PATCH）に追記
6. [ ] `cargo test` 全体で回帰がないことを確認
7. [ ] docs-ai の 02・nested-block-function-and-static の状態を更新
