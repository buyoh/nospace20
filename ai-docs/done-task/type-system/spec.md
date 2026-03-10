# 型システム: 言語仕様

## 型の種類

| 型 | 説明 | 現状 |
|----|------|------|
| `int` | 整数型 (i64) | 内部実装済み、構文なし |
| `void` | 値なし型 | 内部実装済み、構文なし |
| 構造体型 | ユーザー定義の複合型 | 未実装 |
| `&Type` | 参照型 | 未実装 |
| `int[N][]` | 多次元配列型 | 未実装（将来検討） |

## 型注釈構文 (`@`)

`@` 演算子を使用して、式・変数・関数に型を明示する。型注釈は省略可能であり、省略時は既存の型推論が適用される。

### 式への型注釈

```
(1 + 2)@int     # 式の型が int であることを表明 #
1 @ int          # リテラルへの型注釈 #
{1}@int          # ブロック式への型注釈 #
```

- 式の後に `@type` を付けることで、その式の型を明示する。
- 推論された型と注釈が一致しない場合はコンパイルエラー。
- 唯一の例外として `@ void` はキャスト（後述）として機能する。

### 変数宣言への型注釈

```
let: x@int(1);              # int 型変数 #
let: arr@int[3]([1,2,3]);   # int 配列型 (配列 と 型を同時に指定) #
static: s@int(0);           # static 変数にも適用可能 #
final: f@int(42);           # final 変数にも適用可能 #
```

- `let: name@type(init);` の形式で変数宣言時に型を明示する。
- 配列の場合は `name@int[N]` で型と配列サイズを同時に指定する。
  - `let: arr@int[3];` の `type[N]` 表記は `let: arr[3];` と等価。
  - 注釈が付く場合、配列サイズは `@` の後に記述する。`let: arr[3]@int;` は不正。
  - `@` は `type_spec` を貪欲に消費する。`let: x@int[3][5];` は `int[3][5]` 型として解釈される（`let_decl` の配列サイズ `[5]` ではない）。
- 型注釈が省略された場合、現在の動作（常に int）と同じ。
- `@void` での変数宣言（`let: x@void;`）はコンパイルエラー。void 型の変数は定義できない。

### 関数パラメータ・戻り値への型注釈

```
func: add(a@int, b@int)@int {
  return: a + b;
}

func: print_value(x@int)@void {
  __puti(x);
}
```

- 引数: `name@type` の形式で各引数の型を明示する。
- 戻り値: 引数リストの `)` の後に `@type` を付けて戻り値型を明示する。
- 型注釈が省略された場合、引数は暗黙的に `int`、戻り値は本体の `return` 文から推論される。
- 型注釈と推論結果が矛盾する場合はコンパイルエラー。
  - 例: `@int` と注釈しているのに `return` 文がない → エラー
  - 例: `@void` と注釈しているのに `return: 式;` がある → エラー

### 型注釈の優先順位と結合順序

`@`, `.`, `[]` は後置演算子として同じ優先順位レベルに位置し、左から右に結合する。

```
expr_postfix ::=
    | expr_postfix "[" expr "]"     # 配列インデックス
    | expr_postfix "@" type_spec    # 型注釈
    | expr_postfix "." ident        # フィールドアクセス
    | expr_val
```

`@` は `type_spec` を貪欲に消費する。`type_spec` に `[N]` が含まれる場合はそれも型の一部として扱う。

```
x @ MyStruct[3]     # x @ (MyStruct[3]) — MyStruct[3] 型への型注釈
(x @ MyStruct)[3]   # (x @ MyStruct)[3] — 構造体ビュー後に配列アクセス
```

後置演算子の連鎖は左から右に評価される:

```
data @ MyStruct .number      # ((data @ MyStruct).number)
data @ MyStruct .data[0]     # (((data @ MyStruct).data)[0])
```

括弧は任意だが、可読性のために使用を推奨する。

## 明示的キャスト

### void キャスト

任意の `int` 型の式は `@ void` で明示的に void にキャストできる。

```
1 @ void;          # int 値を捨てて void にする #
func_call() @ void;  # 戻り値を明示的に捨てる #
```

- `@ void` は式の値を破棄し、void 型にする。
- int → void のみ許可。void → int は不可（コンパイルエラー）。
- 主な用途: 戻り値を意図的に無視することの表明。

### 参照型キャスト

整数型と参照型の相互キャストが可能。

```
let: addr(100);
let: p@&Point(addr @ &Point);   # int → &Point にキャスト #
let: n(p @ int);                 # &Point → int にキャスト #
```

- `int` → `&Type`: `@ &Type` で参照型にキャスト。値は変更されない（アドレスの再解釈）。
- `&Type` → `int`: `@ int` で整数型にキャスト。値は変更されない。
- `&Type` → `&OtherType`: `@ &OtherType` で異なる参照型にキャスト可能。

### その他のキャスト

- 構造体同士のキャスト: 不可
- int → 構造体: `@ StructName` で再解釈（ポインタキャスト的な用法、後述）
- 構造体 → int: 不可

## 構造体

### 構造体定義

```
struct: MyStruct (number@int, data@int[9]);
```

- `struct: Name (field1@type1, field2@type2, ...);` の形式で構造体を定義する。
- 構造体名は**大文字で始まる**識別子でなければならない。
  - `MyStruct` → OK
  - `myStruct` → コンパイルエラー
  - `_MyStruct` → コンパイルエラー
- フィールドは以下のいずれかの形式で定義する:
  - `name@type` — 型を明示（`@` で型を指定）
  - `name` — 型省略（`int` として扱う）
  - `name[N]` — 型省略の配列（`int[N]` として扱う）
- フィールド型として使用可能な型: `int`, `int[N]` (固定配列), 他の構造体型
  - void 型のフィールドは不可
- 構造体定義はトップレベル（グローバルスコープ）または関数内のスコープに配置可能。
- ホイスティングされる（定義より前に使用可能）。

#### フィールド定義の例

```
# 型を明示する形式 #
struct: MyStruct (number@int, data@int[9]);

# 型を省略する形式（省略時は int）#
struct: MyStruct (number, data[9]);

# 構造体フィールド #
struct: Point (x@int, y@int);
struct: Line (start@Point, end@Point);

# 混在も可能 #
struct: Complex (value, name[16], pos@Point);
```

- `@` 形式は変数宣言の `let: x@Type` と統一的な構文。
- nospace では `:` はキーワード構文の識別に使われるため、フィールド定義では `@` を使用する。

### 構造体変数の宣言と初期化

構造体リテラル式 `struct: Name(values...)` を使用して構造体変数を初期化する。
詳細は [nested-struct-init.md](nested-struct-init.md) を参照。

```
let: s@MyStruct(struct: MyStruct(10, [1,2,3,4,5,6,7,8,9]));
```

- `struct: Name(values...)` は構造体リテラル式。フィールド定義順に値を指定する。
- 初期化値が構造体リテラルの場合、`@Type` 型注釈は省略可能（型がリテラルから推論される）。
- 初期化値の一部または全部を省略できる。省略されたフィールドは未初期化。
- 配列フィールドの初期化には配列初期化構文 `[...]` を使用する。

```
let: s@MyStruct;                                                   # 全フィールド未初期化（型注釈必須）#
let: s(struct: MyStruct(10));                                       # number=10, data は未初期化 #
let: s(struct: MyStruct(10, [1,2,3,4,5,6,7,8,9]));                 # 全フィールド初期化 #
let: s@MyStruct(struct: MyStruct(10, [1,2,3,4,5,6,7,8,9]));        # 型注釈を明示（省略可）#
```

### ネストした構造体の初期化

ネストした構造体フィールドの初期化には、構造体リテラル式をネストする。

```
struct: Point (x@int, y@int);
struct: Line (start@Point, end@Point);

let: line(struct: Line(struct: Point(10, 20), struct: Point(30, 40)));
```

- `struct:` キーワードにより構造体初期化の開始が構文上明確なため、パーサが型情報なしに構造を認識できる。

### フィールドアクセス

```
s.number            # フィールドの読み取り #
s.number = 10;      # フィールドへの代入 #
s.data[0]           # 配列フィールドの要素アクセス #
s.data[i] = 42;     # 配列フィールドの要素への代入 #
```

- `.`（ドット）でフィールドにアクセスする。
- フィールドアクセスは、構造体の先頭からのオフセットに基づく配列アクセスに脱糖される。
- 存在しないフィールド名へのアクセスはコンパイルエラー。

#### 参照型に対するフィールドアクセス

参照型 (`&Type`) に対して `.` でフィールドアクセスを行うと、自動的に逆参照が適用される。

```
struct: Point (x@int, y@int);

let: p(struct: Point(10, 20));
let: ptr@&Point(&p);      # Point への参照 #
ptr.x;                     # (*ptr).x と等価。10 を返す #
ptr.x = 30;                # (*ptr).x = 30 と等価 #
```

- `ref.field` は `(*ref).field` に脱糖される。
- 自動逆参照は最大1回。`&&Point` 型に `.x` は直接適用できない（コンパイルエラー）。
  - `(*ref).x` とすれば `*ref` は `&Point` 型になり、さらに自動逆参照で `(**ref).x` と等価になる。

### 式の型注釈による構造体ビュー

型が `int[N]` の配列に対して `@ StructName` で構造体として再解釈できる。

```
let: data[10];
(data @ MyStruct).number = 10;   # data[0] = 10 と等価 #
(data @ MyStruct).data[0] = 1;   # data[1] = 1 と等価 #
```

- 配列に対する `@ StructName` は、その配列を構造体のメモリレイアウトでアクセスする手段を提供する。
- 配列のサイズが構造体の合計サイズ未満の場合はコンパイルエラー。
- これは型安全なビュー（参照）であり、データのコピーは発生しない。

### 構造体のサイズ

構造体のサイズは全フィールドの合計サイズ。パディングなし。

```
struct: MyStruct (number@int, data@int[9]);
# size = 1 + 9 = 10 スロット #
```

構造体変数は内部的に配列として確保される:
- `let: s@MyStruct;` → `let: s[10];` と等価なメモリ確保

### 構造体の制約

- 構造体は値型ではない。代入 (`s1 = s2`) や関数の引数・戻り値としての受け渡しは直接サポートしない。
  - 構造体の参照(`&s`) やフィールド単位の操作で代替する。
- **構造体変数で別の構造体変数を初期化することは不可。** 構造体は演算やコピーをサポートしない。
  ```
  let: p1(struct: Point(1, 2));
  # let: p2(p1);         コンパイルエラー: 構造体のコピーは不可 #
  # let: p2@Point(p1);   コンパイルエラー: 同上 #
  ```
- ネストした構造体（フィールドが構造体型）はサポートする。アクセスは `s.inner.field` のようにチェーン可能。

```
struct: Point (x, y);
struct: Rect (top_left@Point, bottom_right@Point);

let: r(struct: Rect(struct: Point(1, 2), struct: Point(3, 4)));
r.top_left.x = 10;   # r[0] = 10 と等価 #
r.bottom_right.y = 20;  # r[3] = 20 と等価 #
```

- 再帰的な構造体定義（自身を含む構造体）は不可（コンパイルエラー）。

## 参照型

### 概要

`&Type` は参照型を表す。内部表現は整数（アドレス）と同じだが、型情報を保持する。
参照型の逆参照は対象の `Type` として扱われる。

### 構文

```
let: p@&Point(&some_point);       # Point への参照 #
let: pp@&&Point(&p);              # Point への参照への参照 #
```

- `&Type` で 1 重参照型。
- `&&Type` で 2 重参照型。任意の深さの参照が可能。
- 参照型は `int` と同じスロットサイズ（1 スロット）を持つ。

### 逆参照

```
let: p@&Point(&some_point);
*p;          # Point 型として扱われる（構造体ビュー）#
(*p).x;      # Point のフィールド x にアクセス #
p.x;         # 同上（自動逆参照）#

let: pp@&&Point(&p);
*pp;         # &Point 型 #
**pp;        # Point 型 #
# pp.x;      コンパイルエラー: 自動逆参照は最大1回。&&Point → &Point であり構造体ではない #
(*pp).x;     # (*pp) は &Point 型。さらに自動逆参照で (**pp).x と等価 #
```

### 参照型の演算

参照型と整数型の間で算術演算が可能。演算の種類により結果の型が異なる。

```
let: p@&Point(&some_point);
let: next@&Point(p + 2);    # 2 スロット先のアドレス。&Point 型 #
let: offset(p - p2);         # 同じ参照型同士の減算は int を返す #
```

| 演算 | 結果の型 | 備考 |
|------|----------|------|
| `&Type + int` | `&Type` | アドレスのオフセット加算 |
| `int + &Type` | `&Type` | 同上（可換） |
| `&Type - int` | `&Type` | アドレスのオフセット減算 |
| `&Type - &Type` | `int` | 同じ参照型同士の差分 |
| `&Type + &Type` | — | コンパイルエラー |
| `&A ± &B` (A≠B) | — | コンパイルエラー: 異なる参照型同士の演算 |
| `&Type * int` 等 | — | コンパイルエラー: 乗除算は不可 |

- 比較演算（`==`, `!=`, `<`, `<=`, `>`, `>=`）は参照型同士で使用可能。結果は `int`（0 or 1）。
- 論理演算（`&&`, `||`, `!`）は参照型に適用可能（int と同じ非ゼロ判定）。

## sizeof 式

型のスロットサイズを返すコンパイル時定数式。

```
sizeof: int            # 1 #
sizeof: Point          # 2 (x + y) #
sizeof: MyStruct       # 10 (number + data[9]) #
sizeof: int[5]         # 5 #
sizeof: &Point         # 1 (参照型は常に 1) #
```

- `sizeof: type_spec` の形式で、指定した型のスロット数を返す。
- 結果はコンパイル時定数（constexpr として扱える）。
- `int` = 1, `void` = 0, 構造体 = 全フィールドの合計サイズ, 配列 = 要素数 × 要素サイズ, 参照型 = 1。
- `sizeof:` はキーワード構文（`struct:` と同様にコロン付き）。

## 型仕様 (type_spec) の文法

```bnf
type_spec ::=
    | "int"                        # 整数型
    | "void"                       # 値なし型
    | ident                        # 構造体名（大文字始まり）
    | "&" type_spec                # 参照型
    | type_spec "[" integer "]"    # 配列型（将来: 多次元配列）
```

- `int` と `void` は型コンテキスト (`@` の直後) でのみ型として解釈される。
  通常のコンテキストでは識別子として扱われ、後方互換性を維持する。
- 構造体名は大文字で始まる識別子。通常の識別子と名前空間を共有するが、
  大文字制約により名前衝突のリスクは低い。
- `&` は型コンテキストでは参照型の接頭辞。式コンテキストではアドレス取得演算子（既存）。
  コンテキストにより一意に判別される（`@` の直後は常に型コンテキスト）。

## 文法の変更点まとめ

```bnf
# 追加トークン
token_at ::= "@"

# 型指定
type_spec ::= "int" | "void" | ident | "&" type_spec | type_spec "[" integer "]"

# 式の拡張
expr_postfix ::=
    | expr_val "[" expr "]"
    | expr_val "@" type_spec
    | expr_val "." ident              # 構造体フィールドアクセス
    | expr_val "." ident "[" expr "]" # 配列フィールドアクセス
    | expr_val

# 変数宣言の拡張
let_decl ::= ident ("@" type_spec)? ("[" integer? "]")? ("(" array_init | string_init | expr ")")?
# 注: @type を指定した場合、[N] は type_spec に含まれるため let_decl の [N] は不要

# 関数宣言の拡張
func ::= "func" ":" ident "(" (param ("," param)*)? ")" ("@" type_spec)? block
param ::= ident ("@" type_spec)?

# 構造体定義
struct_decl ::= "struct" ":" ident "(" field_decl ("," field_decl)* ")" ";"
field_decl ::=
    | ident "@" type_spec                   # 型を明示: number@int
    | ident ("[" integer "]")              # 型省略の配列: data[9] (= int[9])
    | ident                                  # 型省略: number (= int)

# 構造体リテラル式
struct_literal ::= "struct" ":" ident "(" (expr ("," expr)*)? ")"

# sizeof 式
sizeof_expr ::= "sizeof" ":" type_spec

# 式の拡張（struct_literal, sizeof を含む）
expr_val ::= ... | struct_literal | sizeof_expr

# グローバル文の拡張
global_stmt ::= ... | struct_decl
stmt ::= ... | struct_decl
```
