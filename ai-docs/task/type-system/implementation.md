# 型システム: 実装設計

モジュールごとの変更点を Phase 別に記載する。

## Phase 1: `@` トークンと型注釈の構文解析

### Step 1-1: Token Parser (`src/token_parser/`)

**変更内容**:

1. `Token` enum に `At` を追加

```rust
pub enum Token {
    // ... 既存 ...
    At,       // @
}
```

2. `Token::describe` に `At` の記述を追加

```rust
Token::At => "'@'".to_string(),
```

3. `parse_to_tokens_internal` で `'@'` を `Token::At` として認識

4. `Keyword` enum に `Struct` を追加

```rust
pub enum Keyword {
    // ... 既存 ...
    Struct,
}
```

`as_keyword_token` に `"struct" => Some(Token::Keyword(Keyword::Struct))` を追加。
`as_str` に `Keyword::Struct => "struct"` を追加。

**影響範囲**: `mod.rs` のみ

### Step 1-2: Tree Parser - 型指定のパース (`src/tree_parser/`)

**変更内容**:

1. 型指定を表す型を追加

```rust
/// 型指定（型注釈に使用）
#[derive(Clone, Debug)]
pub enum TypeSpec {
    Int,
    Void,
    Named(String),                    // 構造体名
    Array(Box<TypeSpec>, usize),      // 配列型 (将来の多次元配列用)
}
```

2. `Expression` enum に型注釈とフィールドアクセスを追加

```rust
pub enum Expression {
    // ... 既存 ...
    /// 型注釈: expr @ type_spec
    TypeAnnotation(Box<LocatedExpression>, TypeSpec),
    /// フィールドアクセス: expr.field_name
    FieldAccess(Box<LocatedExpression>, String),
    /// フィールド配列アクセス: expr.field_name[index]
    FieldArrayAccess(Box<LocatedExpression>, String, Box<LocatedExpression>),
    /// 構造体リテラル: struct: Name(expr, expr, ...)
    StructLiteral(String, Vec<LocatedExpression>),
}
```

3. `Statement` enum に構造体定義を追加

```rust
pub enum Statement {
    // ... 既存 ...
    /// 構造体定義: struct: Name (field: type, ...);
    /// フィールドの型は省略可能（省略時は Int）
    StructDeclaration(String, Vec<StructFieldDecl>),
}

/// 構造体フィールド宣言
#[derive(Clone, Debug)]
pub struct StructFieldDecl {
    pub name: String,
    pub type_spec: Option<TypeSpec>,  // None = int（型省略）
    pub array_size: Option<usize>,    // Some(N) = name[N] 形式
}
```

構造体フィールドのパースは以下の3パターンを認識する:

```
parse_struct_field:
    name = expect ident
    if peek == Token::At:
        # name@type 形式
        consume At
        type_spec = parse_type_spec()
        → StructFieldDecl { name, type_spec: Some(type_spec), array_size: None }
    else if peek == Token::BracketL:
        # name[N] 形式 (= int[N])
        consume BracketL
        size = expect integer
        consume BracketR
        → StructFieldDecl { name, type_spec: None, array_size: Some(size) }
    else:
        # name 形式 (= int)
        → StructFieldDecl { name, type_spec: None, array_size: None }
```

4. `VariableDeclaration` の変更

現在の `VariableDeclaration(String, Box<LocatedExpression>, bool, bool, Option<i64>)` に型注釈フィールドを追加:

```rust
VariableDeclaration(String, Box<LocatedExpression>, bool, bool, Option<i64>, Option<TypeSpec>)
// (name, init_expr, is_static, is_final, array_size, type_annotation)
```

5. `FunctionDeclaration` の変更

引数に型情報を追加、戻り値型を追加:

```rust
FunctionDeclaration(
    String,                          // 関数名
    Vec<(String, Option<TypeSpec>)>, // 引数 (名前, 型注釈)
    Vec<LocatedStatement>,           // 本体
    Option<TypeSpec>,                // 戻り値型注釈
)
```

注: この変更は `TemplateFunctionDefinition` にも同様に適用する必要がある。

6. `ExpressionBuilder` の拡張

`expr_postfix` メソッドを拡張し、`Token::At` のチェックを追加:

```
expr_postfix:
    val = expr_val()
    loop:
        if peek == Token::BracketL:
            消費して index expr を読み、ArrayAccess を生成
        else if peek == Token::At:
            消費して type_spec を読み、TypeAnnotation を生成
        else if peek == Token::Dot:  # (Phase 4 で追加)
            消費して ident を読み、FieldAccess を生成
        else:
            break
    return val
```

`expr_val` メソッドを拡張し、`Keyword(Struct)` を構造体リテラル式として認識:

```
expr_val:
    // ... 既存のケース ...
    if peek == Keyword(Struct):
        consume Keyword(Struct)
        name = expect Identifier (name starts with uppercase)
        expect ParenthesisL
        args = parse_comma_separated_expressions()  # 各引数は通常の式またはネストした struct: Name(...)
        expect ParenthesisR
        → Expression::StructLiteral(name, args)
```

構造体リテラル式は `expr_val` レベルでパースされるため、型情報なしに構文解析可能。
`struct:` キーワードにより、構造体定義（文）と構造体リテラル（式）の区別はコンテキスト（文パーサ vs 式パーサ）で決定される。

構造体リテラル式は `expr_val` レベルでパースされるため、型情報なしに構文解析可能。
`struct:` キーワードにより、構造体定義（文）と構造体リテラル（式）の区別はコンテキスト（文パーサ vs 式パーサ）で決定される。

7. `type_spec` パース関数の追加

```
parse_type_spec():
    if peek == Identifier("int"):
        consume → TypeSpec::Int
    else if peek == Identifier("void"):
        consume → TypeSpec::Void
    else if peek == Identifier(name) && name starts with uppercase:
        consume → TypeSpec::Named(name)
    else:
        error "expected type specifier"
    
    # 配列型チェック (将来拡張)
    while peek == Token::BracketL:
        consume BracketL
        expect integer
        consume BracketR
        wrap in TypeSpec::Array
    
    return type_spec
```

8. `parse_let_decl` の拡張

変数名の後に `Token::At` があれば型注釈を読む:

```
parse_let_decl:
    name = expect ident
    type_annot = None
    if peek == Token::At:
        consume At
        type_annot = parse_type_spec()
    // ... 以降は既存ロジック
```

9. `parse_func` の拡張

引数パース時に `@type` を読み、`)` の後に `@type` があれば戻り値型を読む。

**影響範囲**: `expression/mod.rs`, `statement/mod.rs`, `mod.rs`

## Phase 2: 型注釈の意味解析

### Step 2-1: Semantic Analyzer - 型チェック (`src/semantic_analyzer/`)

**変更内容**:

1. `ValueType` の拡張

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Int,
    Void,
    /// 構造体型。構造体定義のインデックスを保持。
    Struct(usize),
    /// 固定長配列型 (要素型, サイズ)
    /// 現状の int 配列は Array(Int, N) として表現可能
    Array(Box<ValueType>, usize),
}
```

注: `ValueType` が `Copy` を derive しているが、`Struct` と `Array` の追加により `Clone` のみに変更が必要。

2. `Variable` 構造体に型情報を追加

```rust
pub(crate) struct Variable {
    pub slot_index: usize,
    pub is_static: bool,
    pub array_size: Option<usize>,
    pub is_final: bool,
    pub value_type: ValueType,     // 追加: 変数の型
}
```

3. `ExecExpression` に構造体関連バリアントを追加

```rust
pub(crate) enum ExecExpression {
    // ... 既存 ...
    /// 型注釈 (検証済み)
    /// 型チェック通過後、内部的には内側の式と等価。
    /// コード生成では無視可能 (identity operation)。
    TypeAssertion(Box<LocatedExecExpression>, ValueType),
    /// void キャスト
    /// int 式の値を破棄して void にする。
    VoidCast(Box<LocatedExecExpression>),
    /// 構造体フィールドアクセス
    /// (ベース式, フィールドオフセット, フィールドの配列サイズ)
    StructFieldAccess(Box<LocatedExecExpression>, usize, Option<usize>),
    /// 構造体フィールド配列アクセス
    /// (ベース式, フィールドオフセット, インデックス式, 配列サイズ)
    StructFieldArrayAccess(Box<LocatedExecExpression>, usize, Box<LocatedExecExpression>, usize),
}
```

4. TypeAnnotation の変換処理

`expression.rs` の式変換で `Expression::TypeAnnotation` を処理:

```
TypeAnnotation(inner_expr, type_spec):
    exec_inner = 内側の式を変換
    expected_type = type_spec を ValueType に変換
    actual_type = exec_inner.infer_type()
    
    if expected_type == Void && actual_type == Int:
        → ExecExpression::VoidCast(exec_inner)
    else if expected_type != actual_type:
        → コンパイルエラー "type mismatch"
    else:
        → ExecExpression::TypeAssertion(exec_inner, expected_type)
        # または最適化として内側の式をそのまま返す
```

5. 変数宣言の型チェック

`statement.rs` で変数宣言時:
- 型注釈がある場合、初期化式の型と注釈が一致するか検証
- 型注釈を `Variable.value_type` に保存
- 型注釈がない場合、従来通り `Int` を設定

6. 関数宣言の型チェック

- 引数の型注釈: 引数は現在常に `Int`。注釈があれば検証。
- 戻り値の型注釈: 推論結果と注釈の整合性を検証。
  - `@int` 注釈あり & return 文なし → エラー
  - `@void` 注釈あり & `return: expr;` あり → エラー

7. `infer_type` の拡張

`ExecExpression::infer_type` を拡張して新しいバリアントに対応:

```rust
ExecExpression::TypeAssertion(_, vt) => vt.clone(),
ExecExpression::VoidCast(_) => ValueType::Void,
ExecExpression::StructFieldAccess(_, _, None) => ValueType::Int,
ExecExpression::StructFieldAccess(_, _, Some(_)) => ValueType::Int, // 配列フィールド自体は先頭要素
ExecExpression::StructFieldArrayAccess(_, _, _, _) => ValueType::Int,
```

**影響範囲**: `types.rs`, `expression.rs`, `statement.rs`, `scope.rs`, `mod.rs`

## Phase 3: `struct:` 定義

### Step 3-1: 構造体レジストリ

**変更内容**:

1. 構造体定義の内部表現

```rust
/// 構造体のフィールド情報
pub struct StructField {
    pub name: String,
    pub value_type: ValueType,
    pub offset: usize,        // 構造体先頭からのオフセット
    pub size: usize,           // フィールドのサイズ（int=1, array=N）
}

/// 構造体定義
pub struct StructDefinition {
    pub name: String,
    pub fields: Vec<StructField>,
    pub total_size: usize,     // 全フィールドの合計サイズ
}
```

2. `Scope` に構造体レジストリを追加

```rust
pub struct Scope {
    // ... 既存 ...
    struct_definitions: Vec<StructDefinition>,
    struct_name_to_index: BTreeMap<String, usize>,
}
```

3. `Statement::StructDeclaration` の処理

意味解析の Pass 1（ホイスティング）で構造体定義を収集:
- 構造体名が大文字始まりであることを検証
- フィールドの型を解決（他の構造体への参照含む）
- フィールドオフセットと合計サイズを計算
- 再帰的定義のチェック

**影響範囲**: `scope.rs`, `mod.rs`, `types.rs`

## Phase 4: 構造体変数とフィールドアクセス

### Step 4-1: 構造体変数の宣言

- `let: s@MyStruct;` → 内部的に `let: s[total_size];` として確保
- `let: s@MyStruct (val1, val2, ...);` → `let: s[total_size](val1, val2, ...);` として初期化
  - 配列フィールドの初期化値は展開される
- `Variable.value_type` を `ValueType::Struct(index)` に設定

### Step 4-2: フィールドアクセスの解決

tree_parser の `Expression::FieldAccess(expr, field_name)` を意味解析で処理:

1. `expr` の型を推論
2. 型が `ValueType::Struct(idx)` であることを確認
3. `StructDefinition` からフィールドを検索
4. `ExecExpression::StructFieldAccess(base_ref, offset, array_size)` を生成

フィールドアクセスのコード生成:
- `s.number` → `*(addr_of_s + field_offset)` に等価
- `s.data[i]` → `*(addr_of_s + field_offset + i)` に等価

### Step 4-3: 型注釈による構造体ビュー

`(data @ MyStruct).field` の処理:
- `data @ MyStruct` は TypeAnnotation として解析される
- 意味解析で `data` が十分なサイズの配列であることを検証
- `.field` アクセスは通常の構造体フィールドアクセスと同じオフセット計算

**影響範囲**: `expression.rs`, `statement.rs`, `types.rs`

## Phase 5: 明示的キャスト

### void キャスト

`expr @ void` は `ExecExpression::VoidCast(inner)` として表現。

インタプリタ: 内側の式を評価し、値を捨てる。
コンパイラ (compiler_ws): 内側の式をコンパイルし、スタックトップを pop する。

**影響範囲**: `expression.rs` (Phase 2 で一部対応済み)

## ~~Phase 6: ドットアクセスの統合~~ (不要)

名前空間のアクセスに `$` が採用されたため、`.` との衝突はない。
構造体フィールドアクセスの `.` は Phase 4 で完結し、名前空間との統合 Phase は不要。
詳細は [dot-access-conflict.md](dot-access-conflict.md) を参照。

## インタプリタ (`src/interpreter/`) への影響

各 Phase で追加される `ExecExpression` バリアントに対応するハンドラを追加:

| バリアント | 動作 |
|------------|------|
| `TypeAssertion(inner, _)` | 内側の式を評価してそのまま返す |
| `VoidCast(inner)` | 内側の式を評価し、値を捨てる |
| `StructFieldAccess(base, offset, _)` | `*(base_addr + offset)` を評価 |
| `StructFieldArrayAccess(base, offset, idx, _)` | `*(base_addr + offset + idx)` を評価 |

## コンパイラ (`src/compiler_ws/`) への影響

同様に新しいバリアントのコード生成を追加。Whitespace のスタック操作にマッピング。

## オプティマイザ (`src/optimizer/`) への影響

- `TypeAssertion` は最適化パスで削除可能（identity であるため）
- `VoidCast` は dead code として扱えるケースがある
- 構造体フィールドアクセスの定数畳み込み（offset が定数なので、常に定数畳み込み可能）

## テスト計画

### ユニットテスト

- `token_parser`: `@` トークンのパーステスト
- `tree_parser`: 型注釈付き式・変数宣言・関数宣言のパーステスト
- `semantic_analyzer`: 型チェック正常系・異常系

### large テスト (`resources/tests/`)

| テスト | 内容 |
|--------|------|
| `passes/type-annotation-basic.ns` | 基本的な型注釈 |
| `passes/type-annotation-func.ns` | 関数パラメータ・戻り値の型注釈 |
| `passes/type-void-cast.ns` | void キャスト |
| `passes/struct-basic.ns` | 構造体の宣言と使用 |
| `passes/struct-field-access.ns` | フィールドアクセス |
| `passes/struct-nested.ns` | ネストした構造体 |
| `fails/semantic/type-mismatch.ns` | 型不一致エラー |
| `fails/semantic/struct-undefined-field.ns` | 未定義フィールドエラー |
| `fails/syntax/struct-invalid-name.ns` | 構造体名の制約違反 |
