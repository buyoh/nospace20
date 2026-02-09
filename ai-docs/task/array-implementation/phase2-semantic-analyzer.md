# Phase 2: 意味解析 (semantic_analyzer) の変更

## 概要

配列宣言・配列アクセスの意味解析を実装する。
主に Variable 構造体の拡張、スロット数計算の変更、ExecExpression の拡張。

## 変更ファイル

- `src/semantic_analyzer/mod.rs`

## 1. Variable 構造体の拡張

### 変更前

```rust
pub(crate) struct Variable {
    pub identifier: String,
    pub is_static: bool,
}
```

### 変更後

```rust
pub(crate) struct Variable {
    pub identifier: String,
    pub is_static: bool,
    /// 配列サイズ。None なら通常変数（1スロット）、Some(n) なら n スロットの配列。
    pub array_size: Option<usize>,
}
```

## 2. variable_count の計算変更

### 変更前

`ScopeBuilder::build` で:

```rust
let variable_count = self.variables.len();
```

### 変更後

```rust
let variable_count = self.variables.iter()
    .map(|v| v.array_size.unwrap_or(1))
    .sum::<usize>();
```

**注意**: `variable_indices` は変数名 → スロット開始インデックスを表す。
配列変数の場合、次の変数のインデックスは `current_index + array_size` となる。

### variable_indices の構築変更

変更前:
```rust
let mut variable_indices = BTreeMap::new();
for (idx, var) in self.variables.iter().enumerate() {
    variable_indices.insert(var.identifier.clone(), idx);
}
```

変更後:
```rust
let mut variable_indices = BTreeMap::new();
let mut slot_index = 0;
for var in self.variables.iter() {
    variable_indices.insert(var.identifier.clone(), slot_index);
    slot_index += var.array_size.unwrap_or(1);
}
let variable_count = slot_index;
```

## 3. ExecExpression の拡張

### 変更前

```rust
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    Function(String, Vec<Box<ExecExpression>>),
    Factor(i64),
    Variable(IdentifierRef),
}
```

### 変更後

```rust
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    Function(String, Vec<Box<ExecExpression>>),
    Factor(i64),
    Variable(IdentifierRef),
    /// 配列アクセス: (変数参照, インデックス式, 配列サイズ)
    /// 配列サイズは境界チェックに使用
    ArrayAccess(IdentifierRef, Box<ExecExpression>, usize),
}
```

`ArrayAccess(id_ref, index_expr, array_size)`:
- `id_ref`: 配列変数のベースインデックスを指す `IdentifierRef`
- `index_expr`: インデックス計算式
- `array_size`: 配列サイズ（境界チェック用）

## 4. Expression::ArrayAccess の変換

`convert_to_exec_expression_with_resolver` に `ArrayAccess` のケースを追加:

```rust
Expression::ArrayAccess(name, index_expr) => {
    let id_ref = parent_resolver.resolve_variable(name)
        .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?;
    
    // 配列変数であることを確認
    let array_size = /* resolver から Variable の array_size を取得 */;
    let array_size = array_size.ok_or_else(|| {
        vec![code_parse_error!(format!("'{}' is not an array", name))]
    })?;
    
    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;
    
    Ok(Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)))
}
```

### ScopeResolver への array_size 情報の公開

`ScopeResolver::resolve_variable` は現在 `IdentifierRef` のみ返す。
配列情報を取得するには、追加のメソッドが必要:

```rust
impl ScopeResolver {
    /// 変数名から配列サイズ情報を取得
    fn get_array_size(&self, name: &str) -> Option<Option<usize>> {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(&local_index) = scope_info.var_indices.get(name) {
                return Some(scope_info.variables[/* variables内のインデックス */].array_size);
            }
        }
        None
    }
}
```

**問題点**: 現在の `ScopeInfo` は `var_indices: &BTreeMap<String, usize>` を持つが、
これは**スロットインデックス**を返す。配列対応後は `variables` ベクタのインデックスと
スロットインデックスが異なるため、Variable を探すためのマッピングが別途必要。

**解決策**: `ScopeInfo` に `variables_by_name: &BTreeMap<String, usize>` (変数名 → variables ベクタのインデックス) を追加するか、
`ScopeBuilder` に変数名 → Variable ベクタインデックスのマップを別途保持する。

あるいは、より単純に:

```rust
struct ScopeInfo<'a> {
    var_indices: &'a BTreeMap<String, usize>,    // 変数名 → スロットインデックス
    variables: &'a Vec<Variable>,                 // Variable ベクタ
    variable_name_to_idx: &'a BTreeMap<String, usize>,  // 変数名 → variables ベクタ内インデックス
    is_function_scope: bool,
}
```

**より簡潔な方法**: `Variable` ベクタのインデックスは、配列対応前は `var_indices` の値と同じだった。
配列対応後は異なる。`variable_name_to_idx` を追加するか、解析時に `Variable` を名前で線形探索する。

**採用案**: `Scope` に `variable_name_to_var_index: BTreeMap<String, usize>` を追加し、
変数名から `variables` ベクタのインデックスへのマッピングを保持する。
`ScopeInfo` もこの参照を持つ。

## 5. `&arr[i]` の意味解析

既存の `Ref` 処理:

```rust
Expression::Operation1(Operator1::Ref, inner) => {
    match inner.as_ref() {
        Expression::Variable(name) => {
            // → ExecExpression::Operation1(Ref, Variable(id_ref))
        }
        _ => Err("reference operator (&) can only be applied to variables")
    }
}
```

配列対応:

```rust
Expression::Operation1(Operator1::Ref, inner) => {
    match inner.as_ref() {
        Expression::Variable(name) => {
            // → ExecExpression::Operation1(Ref, Variable(id_ref))
        }
        Expression::ArrayAccess(name, index_expr) => {
            // → ExecExpression::Operation1(Ref, ArrayAccess(id_ref, exec_index, arr_size))
        }
        _ => Err("reference operator (&) can only be applied to variables or array elements")
    }
}
```

## 6. 代入時のバリデーション

`Assign` 演算子の左辺値チェック。
現在の意味解析では特に行っていない（インタプリタ側でパニック）。
配列対応後も、代入先のバリデーションは変更不要（インタプリタ側で処理）。

## 7. Variable 宣言時の array_size 伝播

`Statement::VariableDeclaration(name, init_expr, is_static, array_size)` を処理する際:

```rust
Statement::VariableDeclaration(name, _, is_static_explicit, array_size) => {
    scope.add_variable(
        name,
        Variable {
            identifier: name.clone(),
            is_static: final_is_static,
            array_size: array_size.map(|n| n as usize),
        },
    )?;
}
```

## 8. 配列初期化式の処理

Phase 1 で配列初期化は代入文に展開されるため、意味解析での特別処理は不要。
`arr[0] = 10;` などの `Expression::ArrayAccess` は通常の式として処理される。

## 9. テスト項目

### Unit テスト (semantic_analyzer)

- `let: arr[3];` → variable_count == 3, variable_indices["arr"] == 0
- `let: a; let: arr[3]; let: b;` → variable_count == 5, indices: a=0, arr=1, b=4
- `arr[0]` → `ExecExpression::ArrayAccess(IdentifierRef{..}, Factor(0), 3)`
- `arr[0] = 5;` → 正常に解析
- `x[0]` where x is not array → エラー
- `undeclared[0]` → undefined variable エラー
- `&arr[0]` → `Operation1(Ref, ArrayAccess(...))`
- static 配列: `static: arr[3];` → is_static = true

## 10. 考慮事項

### variable_count の整合性

`variable_count` はこれまで「変数の数」を表していたが、
配列対応後は「スロットの総数」を表す。
コメントやドキュメントの更新が必要。

以下の箇所で `variable_count` が使用されている:
- `interpreter/exec.rs`: `enter_block`, `new_func` での `vec![0; scope.variable_count]`
- `compiler_ws/context.rs`: `global_heap_size`, `enter_function`

いずれも「確保する i64 スロットの数」として使用しているため、変更なく配列に対応可能。

### arg_indices の影響

`Function::arg_indices` は引数の変数名 → スロットインデックスの事前計算。
関数引数は通常変数であり配列ではないため、影響なし。
ただし、`variable_indices` の構築変更に伴い、引数のインデックス計算も
新しいスロットベースのインデックスで行う必要がある（引数は先に登録されるため問題なし）。
