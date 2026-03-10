# 型システム: 構造体リテラル式と初期化構文

## 概要

構造体の初期化に `struct: Name(values...)` 構文（構造体リテラル式）を導入する。
これにより、ネストした構造体の初期化を型指向パースなしで実現できる。

## 構造体リテラル式

### 構文

```
struct: Name(expr1, expr2, ...)
```

- `struct:` キーワードに続けて構造体名と `(values...)` を記述する。
- 値はフィールド定義順に対応する。
- 各値は通常の式、配列リテラル `[...]`、またはネストした `struct: Name(...)` を指定できる。
- 値の一部を省略できる。省略されたフィールドは未初期化。

### 例

```
struct: Point (x@int, y@int);
struct: MyStruct (number@int, data@int[9]);
struct: Line (start@Point, end@Point);

# 基本 #
struct: Point(10, 20)

# 配列フィールド #
struct: MyStruct(42, [1,2,3,4,5,6,7,8,9])

# ネスト #
struct: Line(struct: Point(10, 20), struct: Point(30, 40))

# 部分初期化 #
struct: MyStruct(42)   # number=42, data は未初期化 #
```

### 使用可能なコンテキスト

構造体リテラル式は以下のコンテキストでのみ使用可能:

1. **`let:` / `static:` / `final:` の初期化値**
2. **別の構造体リテラル式の中**（ネスト初期化）

構造体は値型ではないため、以下のコンテキストでは使用不可（コンパイルエラー）:

- 関数の引数・戻り値
- 代入式の右辺（`s = struct: Point(1, 2);` は不可）
- 一般の式コンテキスト

## 変数宣言での使用

### 型注釈あり

```
let: s@MyStruct(struct: MyStruct(10, [1,2,3,4,5,6,7,8,9]));
let: line@Line(struct: Line(struct: Point(10, 20), struct: Point(30, 40)));
```

- `let: name@Type(struct: Type(...));` の形式。
- `@Type` と構造体リテラルの型が一致しない場合はコンパイルエラー。

### 型注釈の省略（型推論）

初期化値が構造体リテラル式の場合、`@Type` を省略できる。型はリテラルから推論される。

```
let: s(struct: MyStruct(10, [1,2,3,4,5,6,7,8,9]));
let: line(struct: Line(struct: Point(10, 20), struct: Point(30, 40)));
```

- 構造体リテラルの型名から変数の型と配列サイズが決定される。
- `let: s(struct: MyStruct(...))` は `let: s@MyStruct(struct: MyStruct(...))` と等価。

### 初期化なし

```
let: s@MyStruct;    # 全フィールド未初期化（型注釈必須）#
```

初期化しない場合は型注釈 `@Type` が必須（型を推論する情報がないため）。

## 以前の問題の解決

### パースの独立性

構造体リテラル式は `struct:` キーワードで明示的に開始されるため、パーサは型情報なしにトークン列のみから構造を認識できる。

旧構文の問題:
```
# 旧: パーサが (1, 2) を Point の初期化として解釈するには型情報が必要 #
let: line@Line ((1, 2), (3, 4));
```

新構文の解決:
```
# 新: struct: Point で構造体初期化であることが構文上明確 #
let: line(struct: Line(struct: Point(1, 2), struct: Point(3, 4)));
```

### ホイスティングとの互換性

構造体リテラル式のパースに構造体定義の型情報は不要。パーサは `struct: Name(...)` を構造体名と値リストとして保持するだけで、フィールドとの照合は意味解析フェーズで行う。

## 内部展開

構造体リテラル式は意味解析/コンパイル時にフラットなメモリ初期化に展開される。

```
struct: MyStruct(10, [1,2,3,4,5,6,7,8,9])
# → [10, 1, 2, 3, 4, 5, 6, 7, 8, 9]  (10 スロット)

struct: Line(struct: Point(1, 2), struct: Point(3, 4))
# → [1, 2, 3, 4]  (4 スロット)
```

## ステータス

**決定済み** — `struct: Name(...)` 構文を採用。
