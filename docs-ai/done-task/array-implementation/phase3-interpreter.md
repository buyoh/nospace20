# Phase 3: インタプリタ (interpreter) の変更

## 概要

配列アクセス（読み取り / 代入）の実行ロジック、境界チェック、`&arr[i]` のアドレス計算を実装する。

## 変更ファイル

- `src/interpreter/exec.rs`

## 1. ExecExpression::ArrayAccess の評価

### 読み取り

```rust
ExecExpression::ArrayAccess(id_ref, index_expr, array_size) => {
    let index = try_expr!(self.interpret_expression(index_expr));

    // 境界チェック
    if index < 0 || index >= *array_size as i64 {
        panic!(
            "runtime error: array index out of bounds: index {} but size {}",
            index, array_size
        );
    }

    // ベースアドレス + オフセット でアクセス
    let mut adjusted_ref = *id_ref;
    adjusted_ref.local_index += index as usize;
    ExpressionFlow::Value(self.get_variable(&adjusted_ref))
}
```

**ポイント**: `IdentifierRef` の `local_index` にインデックスを加算することで、
連続スロットの任意の位置にアクセスする。`IdentifierRef` は `Copy` なので
一時コピーを作ってオフセットを加算しても問題ない。

## 2. 代入処理の拡張

### 変更箇所: `interpret_operation2` の `Operator2::Assign` ケース

変更前:
```rust
if let Operator2::Assign = op {
    match expr1.as_ref() {
        ExecExpression::Variable(id_ref) => { /* 変数への代入 */ }
        ExecExpression::Operation1(Operator1::Deref, inner) => { /* *ptr = val */ }
        _ => panic!("runtime error: left value is not assignable"),
    }
}
```

変更後:
```rust
if let Operator2::Assign = op {
    match expr1.as_ref() {
        ExecExpression::Variable(id_ref) => { /* 変数への代入 */ }
        ExecExpression::ArrayAccess(id_ref, index_expr, array_size) => {
            // 配列要素への代入: arr[i] = val
            let index = try_expr!(self.interpret_expression(index_expr));
            let v = try_expr!(self.interpret_expression(expr2));

            // 境界チェック
            if index < 0 || index >= *array_size as i64 {
                panic!(
                    "runtime error: array index out of bounds: index {} but size {}",
                    index, array_size
                );
            }

            let mut adjusted_ref = *id_ref;
            adjusted_ref.local_index += index as usize;
            self.set_variable(&adjusted_ref, v);
            return ExpressionFlow::Value(v);
        }
        ExecExpression::Operation1(Operator1::Deref, inner) => { /* *ptr = val */ }
        _ => panic!("runtime error: left value is not assignable"),
    }
}
```

## 3. `&arr[i]` の参照取得

### 変更箇所: `interpret_operation1` の `Operator1::Ref` ケース

変更前:
```rust
Operator1::Ref => {
    if let ExecExpression::Variable(id_ref) = expr1.as_ref() {
        let addr = self.resolve_address(id_ref);
        ExpressionFlow::Value(addr)
    } else {
        panic!("runtime error: cannot take reference of non-variable");
    }
}
```

変更後:
```rust
Operator1::Ref => {
    match expr1.as_ref() {
        ExecExpression::Variable(id_ref) => {
            let addr = self.resolve_address(id_ref);
            ExpressionFlow::Value(addr)
        }
        ExecExpression::ArrayAccess(id_ref, index_expr, array_size) => {
            let index = try_expr!(self.interpret_expression(index_expr));

            // 境界チェック
            if index < 0 || index >= *array_size as i64 {
                panic!(
                    "runtime error: array index out of bounds: index {} but size {}",
                    index, array_size
                );
            }

            let base_addr = self.resolve_address(id_ref);
            ExpressionFlow::Value(base_addr + index)
        }
        _ => {
            panic!("runtime error: cannot take reference of non-variable");
        }
    }
}
```

## 4. `arr` 単体のアクセス

spec: 「`arr` 単体は `arr[0]` と同義」。

既存の `Variable(id_ref)` の処理は `get_variable(id_ref)` → `scope[id_ref.local_index]`。
配列変数の `local_index` は先頭スロットを指すため、**変更なしで arr[0] と同義の動作をする**。

代入 `arr = 5` も同様に `set_variable(id_ref, 5)` → `scope[id_ref.local_index] = 5` で
先頭要素に代入される。

## 5. 境界チェックの方針

| アクセスパターン | チェック |
|-----------------|---------|
| `arr[i]` 読み取り | `0 <= i < array_size` |
| `arr[i] = val` 代入 | `0 <= i < array_size` |
| `&arr[i]` 参照取得 | `0 <= i < array_size` |
| `*(&arr + i)` 間接アクセス | チェックなし（ポインタ演算は自由） |
| `arr` 単体 | チェック不要（常にインデックス 0） |

## 6. 配列初期化の実行

Phase 1 で tree_parser レベルで代入文に展開されるため、
インタプリタでの特別処理は不要。

```nospace
let: arr[3](10, 20, 30);
```

は以下の文列として実行される:

```
VariableDeclaration("arr", Factor(0), false, Some(3))  → 3スロット確保（0初期化）
Expression(arr[0] = 10)
Expression(arr[1] = 20)
Expression(arr[2] = 30)
```

## 7. テスト項目

### Unit テスト (interpreter)

- 配列宣言・アクセス: `let: arr[3]; arr[0] = 10; arr[1] = 20; assert(arr[0] == 10);`
- 配列初期化: `let: arr[3](10, 20, 30); assert(arr[0] == 10); assert(arr[2] == 30);`
- arr 単体: `let: arr[3]; arr[0] = 42; assert(arr == 42);`
- 境界チェック: `let: arr[3]; arr[3] = 1;` → パニック
- 負のインデックス: `arr[-1]` → パニック
- 参照: `let: arr[3]; let: p; p = &arr; *p = 99; assert(arr[0] == 99);`
- 参照+オフセット: `*((&arr) + 1) = 50; assert(arr[1] == 50);`
- `&arr[i]`: `let: arr[3]; let: p; p = &arr[1]; *p = 77; assert(arr[1] == 77);`
- 式インデックス: `let: arr[3]; let: i; i = 1; arr[i] = 42; assert(arr[i] == 42);`
- static 配列: `static: arr[3]; arr[0] = 5; assert(arr[0] == 5);`

### Large テスト (統合テスト)

- 配列の基本操作テスト: `resources/tests/` に `.ns` + `.check.json` を追加
- ループ内での配列操作
- 配列を引数のように使う（参照渡し相当）

## 8. 考慮事項

### パフォーマンス

境界チェックは各アクセスごとに実行される。
大量の配列アクセスがあるプログラムではオーバーヘッドとなるが、
nospace の主な用途を考えると問題ないと判断。

### エラーメッセージ

境界外アクセスのエラーメッセージには、配列名・インデックス・サイズを含める。
ただし、`IdentifierRef` から配列名を取得するには追加の情報が必要。
簡易的には `index` と `array_size` のみをメッセージに含める。
