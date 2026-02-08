# TODO: 複数変数宣言の実装

## 概要

`test_legacy_015`, `test_legacy_020`, `test_legacy_023` が失敗している。`let:a, b;` という形式の複数変数宣言がパーサーでサポートされていないため。

## 問題の詳細

### エラー内容

```
error: unexpected token: expected Token::Semicolon
  (internal: src/tree_parser/statement/mod.rs:53)
line:4 column:10
    let:r1, r2;
          ^
```

### 原因

- パーサーが `let:` 文で複数の変数名をカンマ区切りで宣言することをサポートしていない
- 現在は `let:a;` のように1つの変数のみ宣言可能

### 仕様

spec.md の記載:

```
let: a(1), b(2);     # 複数変数を初期化して宣言 #
```

ただし、spec.md のセクション 4.1 には「### 4.1 変数の初期化 (未実装)」と明記されている。

### 失敗しているテストケース

1. **test_legacy_015**: `let:r1, r2;`
2. **test_legacy_020**: `let:n,x;`
3. **test_legacy_023**: `let:a,b;`

### テストケース例

`resources/tests/passes/legacy/legacy_015.ns`:

```nospace
func:fibo(x, ret){
    let:r1, r2;
    if: (x <= 1) {
        *ret = 1;
    }
    else: {
        fibo(x-1, &r1);
        fibo(x-2, &r2);
        *ret = r1 + r2;
    };
}
```

## 実装に必要な作業

### パーサー側

1. `src/tree_parser/statement/mod.rs` で `let:` 文のパース処理を修正
   - カンマ区切りで複数の変数名を受け付けるようにする
   - オプションで初期化式 `(expr)` も受け付ける

2. AST (抽象構文木) の修正
   - `Statement::Let` が複数の変数を保持できるようにする
   - または、複数の `Statement::Let` に展開する

### セマンティック解析側

1. 複数変数宣言を処理できるように修正
   - スコープに複数の変数を登録

### インタプリタ/コンパイラ側

1. 複数変数宣言の処理を実装
   - 複数のスタック領域確保
   - 初期化式がある場合の処理

## 優先度

中 - spec.md で未実装と明記されているが、legacy テストで使用されている

## 備考

- 初期化なしの複数変数宣言 `let:a, b;` と初期化ありの複数変数宣言 `let:a(1), b(2);` の両方をサポートする必要がある
- 初期化なしの場合は、単純に複数の変数領域を確保するだけ
