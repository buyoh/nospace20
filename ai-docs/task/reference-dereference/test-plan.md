# テスト計画

## テスト分類

### 1. ユニットテスト

各モジュール内の `#[cfg(test)]` で実装。

#### token_parser

| テスト | 入力 | 期待結果 |
|--------|------|---------|
| 単独 `&` | `&` | `Token::Ampersand` |
| `&&` | `&&` | `Token::DoubleAmpersand`（既存動作維持） |
| `&x` | `&x` | `[Token::Ampersand, Token::Identifier("x")]` |

#### tree_parser

| テスト | 入力 | 期待結果 |
|--------|------|---------|
| 参照 | `&x` | `Operation1(Ref, Variable("x"))` |
| デリファレンス | `*p` | `Operation1(Deref, Variable("p"))` |
| 二重デリファレンス | `**p` | `Operation1(Deref, Operation1(Deref, Variable("p")))` |
| 乗算との共存 | `a * b` | `Operation2(Multiply, Variable("a"), Variable("b"))` |
| 乗算+デリファレンス | `a * *p` | `Operation2(Multiply, Variable("a"), Operation1(Deref, Variable("p")))` |
| デリファレンス代入 | `*p = 5` | `Operation2(Assign, Operation1(Deref, Variable("p")), Factor(5))` |
| 参照+算術 | `&x + 1` | パースエラー（`+` の左辺が `&x` で型不整合だが、構文上は有効→意味解析で判断） |

#### semantic_analyzer

| テスト | 入力 | 期待結果 |
|--------|------|---------|
| 変数の参照 | `let: x; &x;` | `Operation1(Ref, Variable(IdentifierRef{...}))` |
| リテラルの参照 | `&5;` | 意味解析エラー |
| 式の参照 | `&(x + 1);` | 意味解析エラー |
| デリファレンス | `let: p; *p;` | `Operation1(Deref, Variable(IdentifierRef{...}))` |

### 2. 統合テスト（Large テスト）

`resources/tests/` に配置。`__trace` / `__assert` を使用して検証。

#### 基本テスト

```nospace
# resources/tests/ref_basic.ns
func: main() {
    let: x; let: p;
    x = 42;
    p = &x;
    __assert(*p == 42);
    __trace(1);
    return: 0;
}
```

```json
{ "trace": [[1, 1]] }
```

#### デリファレンス代入テスト

```nospace
# resources/tests/ref_deref_assign.ns
func: main() {
    let: x; let: p;
    x = 10;
    p = &x;
    *p = 20;
    __assert(x == 20);
    __trace(1);
    return: 0;
}
```

```json
{ "trace": [[1, 1]] }
```

#### 関数引数ポインタ渡しテスト

```nospace
# resources/tests/ref_func_arg.ns
func: set_value(ptr, val) {
    *ptr = val;
}

func: main() {
    let: x;
    x = 0;
    set_value(&x, 100);
    __assert(x == 100);
    __trace(1);
    return: 0;
}
```

```json
{ "trace": [[1, 1]] }
```

#### スワップ関数テスト

```nospace
# resources/tests/ref_swap.ns
func: swap(a, b) {
    let: tmp;
    tmp = *a;
    *a = *b;
    *b = tmp;
}

func: main() {
    let: x; let: y;
    x = 10;
    y = 20;
    swap(&x, &y);
    __assert(x == 20);
    __assert(y == 10);
    __trace(1);
    return: 0;
}
```

```json
{ "trace": [[1, 1]] }
```

#### 二重ポインタテスト

```nospace
# resources/tests/ref_double.ns
func: main() {
    let: x; let: p; let: pp;
    x = 99;
    p = &x;
    pp = &p;
    __assert(**pp == 99);
    __trace(1);
    return: 0;
}
```

```json
{ "trace": [[1, 1]] }
```

#### エラーテスト

```nospace
# resources/tests/ref_error_literal.ns
# &5 のような不正な参照
func: main() {
    &5;
    return: 0;
}
```

```json
{ "compile_error": true }
```

### 3. Whitespace コンパイラテスト

上記の統合テストのうち、IO を使用しないものは compile_test.rs でも実行される（既存のテスト基盤で自動的に対応）。

## テスト実行方法

```bash
# ユニットテストのみ
cargo test --lib

# 統合テスト（Large テスト）のみ
cargo test --test code_test

# 全テスト
cargo test

# Whitespace コンパイラテスト
cargo test --test compile_test
```
