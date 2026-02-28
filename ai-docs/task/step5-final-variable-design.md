# Step 5: final 変数 詳細設計

## 概要

再代入不可の変数 `final: name(expr);` を実装する。
constexpr とは異なり、ランタイム値を保持できる実体のある変数であり、スタックスロットを確保する。
再代入をコンパイル時に検出してエラーにする。

**親ドキュメント**: [unimplemented-variable-features.md](unimplemented-variable-features.md) §3, §4.6

---

## 1. 言語仕様確認

`docs/spec.md` より:
```
- (未実装) `final` 再代入不可。`const` リテラルのみ代入可かつ再代入不可。
```

```nospace
func: foobar(a, b) {
  final: x(a + b);
  const: y(57);
  # x = 57; コンパイルエラー #
  &x;
  # &y; コンパイルエラー #
}
```

**注意**: `const` は既に `constexpr` として実装済み（コンパイル時定数、スロット確保なし）。
`final` は別物であり、ランタイム値を保持する変数で再代入を禁止するもの。

---

## 2. パーサー変更（token_parser）

### 2.1 キーワード追加

```rust
// src/token_parser/mod.rs
pub enum Keyword {
    // ... 既存 ...
    Final,
}

fn as_keyword_token(s: &str) -> Option<Token> {
    match s {
        // ... 既存 ...
        "final" => Some(Token::Keyword(Keyword::Final)),
        _ => None,
    }
}
```

---

## 3. パーサー変更（tree_parser）

### 3.1 Statement::VariableDeclaration の拡張

現在:
```rust
VariableDeclaration(String, Box<LocatedExpression>, bool, Option<i64>)
// (name, init_expr, is_static, array_size)
```

**方針**: `is_final: bool` フラグを追加する。

```rust
VariableDeclaration(String, Box<LocatedExpression>, bool, bool, Option<i64>)
// (name, init_expr, is_static, is_final, array_size)
```

**代替案**: フラグ構造体を導入する。

```rust
#[derive(Clone, Debug)]
pub struct VarDeclFlags {
    pub is_static: bool,
    pub is_final: bool,
}

VariableDeclaration(String, Box<LocatedExpression>, VarDeclFlags, Option<i64>)
```

**推奨**: まずは bool 追加で実装し、将来のフラグ増加時に構造体化を検討する。
`is_final` を第4引数に挿入し、既存の `Option<i64> (array_size)` を第5引数にシフトする。

### 3.2 パース処理の分岐

```rust
// src/tree_parser/statement/mod.rs の parse_to_statements()
Token::Keyword(Keyword::Final) => {
    statements.extend(self.parse_to_statements_final_variable(start_pos));
    continue;
}
```

#### 新規関数: `parse_to_statements_final_variable`

```rust
/// `final:` キーワードを消費して final 変数宣言をパースする。
fn parse_to_statements_final_variable(
    &mut self,
    start_pos: usize,
) -> Vec<LocatedStatement> {
    self.iter.next(); // Final キーワードを消費
    self.parse_variable_declarations_with_final(start_pos, false, true)
}
```

**実装方法の選択肢**:

A) `parse_variable_declarations` に `is_final` パラメータを追加する
   - 変更が最小限
   - `parse_variable_declarations(start_pos, is_static, is_final)` のように拡張

B) `parse_to_statements_variable` を拡張して `is_final` を渡す
   - 既存の `parse_to_statements_variable(start_pos, is_static)` の呼び出し元を変更

**推奨**: A) `parse_variable_declarations` に `is_final` パラメータを追加する。
`let:` と `static:` の場合は `is_final=false`、`final:` の場合は `is_final=true` で呼び出す。

```rust
fn parse_to_statements_variable(
    &mut self,
    start_pos: usize,
    is_static: bool,
) -> Vec<LocatedStatement> {
    self.iter.next();
    self.parse_variable_declarations(start_pos, is_static, false) // is_final=false
}

fn parse_to_statements_final_variable(
    &mut self,
    start_pos: usize,
) -> Vec<LocatedStatement> {
    self.iter.next();
    self.parse_variable_declarations(start_pos, false, true) // is_final=true
}
```

`parse_variable_declarations` 内で、`VariableDeclaration` の生成時に `is_final` を渡す。

### 3.3 final + static の組み合わせ

`final:` と `static:` は独立したキーワードで宣言するため、`static final` や `final static` の組み合わせは初回実装では**サポートしない**。

- `final: x(10);` → final 変数（非 static）
- `static: x(10);` → static 変数（非 final）
- `static final: x(10);` → 構文エラー（未サポート）

将来的に必要であれば、修飾子の組み合わせを検討する。

### 3.4 final 変数の初期値

`final:` 変数は初期値の有無を問わず宣言可能：
- `final: x(10);` → 初期値あり（以降の代入はすべてエラー）
- `final: x;` → 初期値なし（1回のみ代入可能）

ただし、初期値なしの場合の「1回のみ代入可能」の静的検証は複雑であるため、
初回実装では **初期値ありの final のみ**をサポートし、以降の全代入をエラーとする。

初期値なしの final は将来拡張として扱う。

### 3.5 final 変数の配列サポート

`final: arr[3]([1, 2, 3]);` のような配列の final 宣言について：
- 配列要素への代入 `arr[0] = 10;` もエラーとすべき
- 初回実装ではサポートし、配列全体を再代入不可とする

---

## 4. 意味解析変更（semantic_analyzer）

### 4.1 Variable 構造体の拡張

```rust
// src/semantic_analyzer/types.rs
pub(crate) struct Variable {
    pub slot_index: usize,
    pub is_static: bool,
    pub array_size: Option<usize>,
    /// final フラグ。true の場合、初期値設定後は再代入不可。
    pub is_final: bool,  // 追加
}
```

### 4.2 Pass 1b: 変数登録時に is_final を設定

```rust
// パス1b: 変数宣言収集
Statement::VariableDeclaration(name, _, is_static_explicit, is_final, array_size) => {
    let final_is_static = *is_static_explicit || is_static;
    scope.add_variable(
        name,
        Variable {
            slot_index: 0,
            is_static: final_is_static,
            array_size: array_size.map(|n| n as usize),
            is_final: *is_final,  // 追加
        },
    )?;
}
```

### 4.3 ScopeResolver への is_final チェックメソッド追加

```rust
impl<'a> ScopeResolver<'a> {
    /// 変数が final かどうかを返す
    ///
    /// スコープスタックを内側から外側へ探索し、
    /// 変数情報を取得して is_final フラグを返す。
    /// 変数が見つからない場合は false を返す。
    pub fn is_final_variable(&self, name: &str) -> bool {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(&var_idx) = scope_info.var_name_to_var_index.get(name) {
                return scope_info.variables[var_idx].is_final;
            }
        }
        false
    }
}
```

### 4.4 代入式での final チェック

`convert_to_exec_expression_with_resolver` 内の `Expression::Operation2` 処理で、
代入演算子のターゲットが final 変数の場合にコンパイルエラーを報告する。

**チェックポイント**: 複合代入演算子 (`+=`, `-=`, `*=`, `/=`, `%=`) は既に
`Assign` + 二項演算に展開されるため、`Assign` のみチェックすれば十分。

```rust
Expression::Operation2(op, l, r) => {
    // 複合代入演算子の展開（既存処理）
    let (actual_op, actual_l, actual_r) = match op {
        // ... 既存の展開処理 ...
    };

    // final 変数への代入チェック（Assign の場合のみ）
    if actual_op == Operator2::Assign {
        match &actual_l.expression {
            Expression::Variable(name) => {
                let resolved_name = parent_resolver.resolve_alias_chain(name)
                    .map_err(|e| vec![code_parse_error!(loc.start, e)])?;
                if parent_resolver.is_final_variable(&resolved_name) {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        format!("cannot assign to final variable '{}'", name)
                    )]);
                }
            }
            Expression::ArrayAccess(name, _) => {
                let resolved_name = parent_resolver.resolve_alias_chain(name)
                    .map_err(|e| vec![code_parse_error!(loc.start, e)])?;
                if parent_resolver.is_final_variable(&resolved_name) {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        format!("cannot assign to element of final array '{}'", name)
                    )]);
                }
            }
            _ => {
                // *ptr = value のようなケースは静的チェック不可
                // final 変数への間接的な書き込みは検出できない
            }
        }
    }

    // 以降はオペランドの変換（既存処理）
    // ...
}
```

### 4.5 VariableDeclaration の初期化式チェック

final 変数の宣言時の初期化式はそのまま処理する（通常の変数宣言と同じ）。
初期化式自体は「代入」ではなく「初期化」であるため、final チェックの対象外。

現在の Pass 2 での VariableDeclaration 処理:
```rust
Statement::VariableDeclaration(_, init, is_static_explicit, _) => {
    let exec_stmt = ExecStatement::Expression(
        convert_to_exec_expression_with_resolver(init, &resolver, &effective_func_return_types)?
    );
    // ...
}
```

この処理は変更不要。初期化式は `Assign` 演算子を含むが、
この代入はパーサー段階で生成されるもの（`let: x(10)` → `x = 10` 相当の AST）であり、
final チェックを回避する必要がある。

**重要**: `parse_variable_declarations` で生成される初期化式の形を確認する必要がある。
初期化式は `Expression::Operation2(Assign, Variable(name), init_value)` の形で生成されている場合、
final チェックが初期化式にも適用されてしまう。

この問題への対処方法:
- **方法A**: Pass 2 で VariableDeclaration を処理する際、一時的に final フラグを無効化する
- **方法B**: 初期化式の代入は `Assign` ではなく別の形で表現する（既存の VariableDeclaration の init は代入式ではなく初期化値のみ）
- **方法C**: `VariableDeclaration` の処理で final 変数の初期化代入をスキップするフラグを使う

**確認事項**: 現在の `VariableDeclaration` の `init_expr` が実際にどのような AST かを確認する。
`let: x(10);` の場合:
- `VariableDeclaration("x", Box(Assign(Variable("x"), Factor(10))), false, None)` なのか
- `VariableDeclaration("x", Box(Factor(10)), false, None)` なのか

→ パーサーコード（`parse_variable_init()` L592-627）を確認した結果、
初期化式は `x = expr` 形式の代入式として構築される。
つまり `init_expr` は `Assign(Variable("x"), ...)` を含む。
初期値なしの場合は `Factor(0)` （代入なしの値）。

**対処**: VariableDeclaration の初期化式変換を特別扱いする。

```rust
Statement::VariableDeclaration(name, init, is_static_explicit, is_final, _) => {
    // final 変数の初期化式は代入としてチェックしない
    // → 一時的に final フラグを「初期化済み」としてマークしない
    // → convert_to_exec_expression_with_resolver に「初期化代入を許可」フラグを渡す

    // 対処法: final チェックの対象から初期化代入を除外する
    // 初期化式 `x = <init>` で lhs が宣言中の変数自身なら許可
    // ...
}
```

**もっとシンプルな対処**:

Pass 1b で収集される変数テーブルには全ての final 変数が含まれるが、
Pass 2 の VariableDeclaration 処理順は宣言順。
VariableDeclaration の初期化式変換時には、宣言中の変数名を「final チェック除外リスト」に記録しておく。

しかし、これは `convert_to_exec_expression_with_resolver` に除外リストを渡す必要がありシグネチャ変更が大きい。

**最もシンプルな対処**: 初期化式のトップレベルの Assign を直接識別して許可する。

```rust
Statement::VariableDeclaration(name, init, is_static_explicit, is_final, _) => {
    // 初期化式を変換（hoisting 済みのため、ここで final チェックは行わない）
    // VariableDeclaration の init は必ず "name = expr" 形式
    // この代入は「初期化」であり final チェック対象外
    //
    // 対処: init expr の変換時のみ final チェックを回避する
    // → init expr のルートが Assign(Variable(name), rhs) の場合、
    //   rhs のみ convert して ExecExpression::Operation2(Assign, Variable(ref), rhs) を構築
    //   final チェックをバイパス
}
```

**推奨アプローチ**: VariableDeclaration の初期化式変換を特別扱いする。

初期化式は必ず `Assign(Variable(name), init_value)` の形式であるため、
この代入を分解して rhs だけ変換し、lhs は resolver で解決して直接 `ExecExpression::Operation2(Assign, ...)` を構築する。
これにより、`convert_to_exec_expression_with_resolver` の再帰呼び出しで Assign チェックが走ることを回避する。

もしくは、もっと単純に：`is_initializing: bool` フラグを `convert_to_exec_expression_with_resolver` に追加せず、
**VariableDeclaration の処理で明示的に `init_expr` の Assign をスキップする**。

```rust
// init が Assign(Variable(name), rhs) の場合、rhs だけを convert
if let Expression::Operation2(Operator2::Assign, lhs, rhs) = &init.expression {
    // lhs (Variable(name)) は resolver で解決
    // rhs は convert_to_exec_expression_with_resolver で変換
    // → ExecExpression::Operation2(Assign, exec_lhs, exec_rhs) を直接構築
    // final チェックなし（初期化のため）
}
```

---

## 5. テストケース

### 成功ケース

#### var_final_001
基本的な final 変数。
```nospace
func: __main() {
  __trace(0);
  final: x(10);
  __assert(x == 10);
}
```

#### var_final_002
final 変数にランタイム値を設定。
```nospace
func: add(a, b) { return: a + b; }
func: __main() {
  __trace(0);
  final: x(add(3, 7));
  __assert(x == 10);
}
```

### エラーケース

#### var_final_reassign_001
final 変数への再代入でコンパイルエラー。
```nospace
func: __main() {
  final: x(10);
  x = 20;
}
```
期待: コンパイルエラー（cannot assign to final variable 'x'）

#### var_final_compound_assign_001
複合代入演算子での再代入もエラー。
```nospace
func: __main() {
  final: x(10);
  x += 5;
}
```
期待: コンパイルエラー

#### var_final_array_001
final 配列の要素への代入もエラー。
```nospace
func: __main() {
  final: arr[3]([1, 2, 3]);
  arr[0] = 10;
}
```
期待: コンパイルエラー（cannot assign to element of final array 'arr'）

### 既存テストケース

`resources/tests/passes/variables/disabled_var_final_001.ns` が既に存在する。
このテストケースを有効化（ファイル名から `disabled_` を除去）して使用する。

```nospace
# [未実装] final 変数(再代入不可) #
func: __main() {
  __trace(0);
  final:x;
  x = 10;
  __assert(x == 10);
  # x = 20;  # エラー: 再代入不可 #
}
```

**注意**: このテストは初期値なしの `final: x;` を使用しており、
初期値なし final をサポートする場合にのみ有効化できる。
初期値なし final を初期実装でサポートしない場合、テスト内容の修正が必要。

---

## 6. interpreter / compiler_ws への影響

変更不要。final チェックは意味解析でのみ行われ、
生成される `ExecExpression` は通常の変数と同じ構造を持つ。

---

## 7. 変更ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `src/token_parser/mod.rs` | `Keyword::Final` 追加、`as_keyword_token` に `"final"` 追加 |
| `src/tree_parser/statement/mod.rs` | `VariableDeclaration` に `is_final` フラグ追加、`final:` パース処理追加 |
| `src/semantic_analyzer/types.rs` | `Variable` 構造体に `is_final: bool` 追加 |
| `src/semantic_analyzer/scope.rs` | `ScopeResolver::is_final_variable()` 追加 |
| `src/semantic_analyzer/mod.rs` | Pass 1b で `is_final` 設定、Pass 2 の代入チェックに final 検証追加 |
| `resources/tests/passes/variables/` | テストケース追加・有効化 |

---

## 8. 注意点・リスク

1. **初期化式の final バイパス**: VariableDeclaration の初期化式変換で final チェックを回避する処理が最も注意を要する。§4.5 の推奨アプローチを慎重に実装する。

2. **VariableDeclaration のフィールド数増加**: 5つの要素を持つタプルは可読性が低い。将来的にフラグ構造体への移行を検討。

3. **間接的な final 変数の変更**: `&final_var` でアドレスを取得し、`*ptr = value` で書き込むことは検出できない。これは仕様上の制約であり、初回実装では許容する。

4. **既存テストへの影響**: `VariableDeclaration` の引数追加により、パターンマッチしている全箇所を更新する必要がある。変更箇所の網羅的な確認が必要。
