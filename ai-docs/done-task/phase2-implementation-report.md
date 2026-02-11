# Phase 2: 意味解析 (semantic_analyzer) 実装報告

## 実装日

2026年2月10日

## 概要

配列機能の Phase 2（意味解析）を実装しました。`Variable` 構造体の拡張、スロット計算の変更、`ExecExpression` の拡張を行いました。

## 実装内容

### 1. Variable 構造体の拡張

```rust
pub(crate) struct Variable {
    pub identifier: String,
    pub is_static: bool,
    /// 配列サイズ。None なら通常変数（1スロット）、Some(n) なら n スロットの配列。
    pub array_size: Option<usize>,
}
```

### 2. variable_count とvariable_indices の計算変更

配列のサイズを考慮したスロット計算を実装:

```rust
fn build(self, is_function_scope: bool, root_statements: Vec<ExecStatement>) -> Scope {
    let mut variable_indices = BTreeMap::new();
    let mut variable_name_to_var_index = BTreeMap::new();
    let mut slot_index = 0;
    for (var_idx, var) in self.variables.iter().enumerate() {
        variable_indices.insert(var.identifier.clone(), slot_index);
        variable_name_to_var_index.insert(var.identifier.clone(), var_idx);
        slot_index += var.array_size.unwrap_or(1);
    }
    let variable_count = slot_index;
    // ...
}
```

### 3. Scope 構造体の拡張

変数名から variables ベクタ内のインデックスへのマッピングを追加:

```rust
pub struct Scope {
    pub(crate) variable_indices: BTreeMap<String, usize>,
    /// 配列対応のため追加
    pub(crate) variable_name_to_var_index: BTreeMap<String, usize>,
    pub(crate) variables: Vec<Variable>,
    pub(crate) variable_count: usize,
    // ...
}
```

### 4. ExecExpression の拡張

配列アクセスを表す新しいバリアントを追加:

```rust
pub(crate) enum ExecExpression {
    // ...
    /// 配列アクセス: (変数参照, インデックス式, 配列サイズ)
    ArrayAccess(IdentifierRef, Box<ExecExpression>, usize),
}
```

### 5. ScopeResolver の拡張

配列サイズを取得する新しいメソッドを追加:

```rust
impl<'a> ScopeResolver<'a> {
    fn get_array_size(&self, name: &str) -> Option<Option<usize>> {
        // 変数名から配列サイズ情報を取得
    }
}
```

### 6. Expression::ArrayAccess の変換

配列アクセスの意味解析を実装:

```rust
Expression::ArrayAccess(name, index_expr) => {
    let id_ref = parent_resolver.resolve_variable(name)?;
    let array_size = parent_resolver.get_array_size(name)?
        .ok_or_else(|| vec![code_parse_error!(format!("'{}' is not an array", name))])?;
    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;
    Ok(Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)))
}
```

### 7. `&arr[i]` の意味解析

参照演算子の対象として配列要素をサポート:

```rust
Expression::Operation1(Operator1::Ref, inner) => {
    match inner.as_ref() {
        Expression::Variable(name) => { /* 既存の処理 */ }
        Expression::ArrayAccess(name, index_expr) => {
            // 配列要素への参照を生成
            Ok(Box::new(ExecExpression::Operation1(
                Operator1::Ref,
                Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)),
            )))
        }
        _ => Err(/* エラー */)
    }
}
```

### 8. 変数宣言時の array_size 伝播

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

### 9. コンパイラとインタプリタの対応

Phase 3/4 での実装に備え、`ArrayAccess` のケースを追加:

- `compiler_ws/expression.rs`: エラーを返す実装
- `interpreter/exec.rs`: パニックする実装

## テスト結果

### 追加したユニットテスト（全て通過）

1. `test_success_array_declaration` - 配列宣言
2. `test_success_multiple_variables_with_array` - 複数変数と配列の混在
3. `test_success_array_access` - 配列アクセス
4. `test_success_array_assignment` - 配列への代入
5. `test_error_array_access_non_array` - 配列でない変数への配列アクセス
6. `test_error_array_access_undefined` - 未定義変数への配列アクセス
7. `test_success_ref_array_element` - 配列要素への参照
8. `test_success_static_array` - static 配列

全てのテストが通過しました。

### 既存のテスト

- semantic_analyzerのユニットテスト: 全て通過 (20 passed)
- 全体のテスト: 96 passed, 5 failed, 14 ignored

失敗しているテストは、ネストされた関数宣言を含む `scope_static_*` 関連のテストで、配列実装とは無関係です。これらは元々失敗していたテストです。

## 変更ファイル

- `src/semantic_analyzer/mod.rs` - 意味解析の変更
- `src/compiler_ws/expression.rs` - ArrayAccess ケースの追加（未実装エラー）
- `src/interpreter/exec.rs` - ArrayAccess ケースの追加（パニック）

## 次のステップ

Phase 3: インタプリタでの配列アクセス・代入・初期化の実装
