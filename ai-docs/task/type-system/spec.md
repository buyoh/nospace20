# 型システム: 言語仕様

## 型の種類

| 型 | 説明 | 現状 |
|----|------|------|
| `int` | 整数型 (i64) | 内部実装済み、構文なし |
| `void` | 値なし型 | 内部実装済み、構文なし |
| 構造体型 | ユーザー定義の複合型 | 未実装 |
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
- 型注釈が省略された場合、現在の動作（常に int）と同じ。

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

### 型注釈の優先順位

`@` は後置演算子として扱い、配列アクセス `[]` と同じ優先順位レベルに位置する。

```
expr_postfix ::=
    | expr_val "[" expr "]"       # 配列インデックス
    | expr_val "@" type_spec      # 型注釈
    | expr_val
```

ただし、`expr_postfix "@" type_spec "[" expr "]"` のように `@` と `[]` を連鎖させることはできない（構造体フィールドアクセスで `()` を使う場合を除く）。

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

### その他のキャスト

- 構造体同士のキャスト: 不可
- int → 構造体: `@ StructName` で再解釈（ポインタキャスト的な用法、後述）
- 構造体 → int: 不可

## 構造体

### 構造体定義

```
struct: MyStruct (number: int, data: int[9]);
```

- `struct: Name (field1: type1, field2: type2, ...);` の形式で構造体を定義する。
- 構造体名は**大文字で始まる**識別子でなければならない。
  - `MyStruct` → OK
  - `myStruct` → コンパイルエラー
  - `_MyStruct` → コンパイルエラー
- フィールドは以下のいずれかの形式で定義する:
  - `name: type` — 型を明示
  - `name` — 型省略（`int` として扱う）
  - `name[N]` — 型省略の配列（`int[N]` として扱う）
  - `name @ StructName` — フィールドが構造体型
- フィールド型として使用可能な型: `int`, `int[N]` (固定配列), 他の構造体型
  - void 型のフィールドは不可
- 構造体定義はトップレベル（グローバルスコープ）または関数内のスコープに配置可能。
- ホイスティングされる（定義より前に使用可能）。

#### フィールド定義の例

```
# 型を明示する形式 #
struct: MyStruct (number: int, data: int[9]);

# 型を省略する形式（省略時は int）#
struct: MyStruct (number, data[9]);

# 構造体フィールド #
struct: Point (x: int, y: int);
struct: Line (start @ Point, end @ Point);
# ↑ は以下と等価 #
struct: Line (start: Point, end: Point);

# 混在も可能 #
struct: Complex (value, name[16], pos @ Point);
```

- `:` 形式と `@` 形式は同じ意味。`name: StructName` と `name @ StructName` は等価。
- `@` 形式は、変数宣言の `let: x@Type` と視覚的に一貫性がある。
- `:` は構造体のフィールド定義でのみ型指定に使える（nospace では `:` はキーワード構文の識別に使われるが、構造体フィールド定義の `name: type` は文脈上一意に判別可能）。

### 構造体変数の宣言と初期化

```
let: s@MyStruct (10, [1,2,3,4,5,6,7,8,9]);
```

- `let: name@StructName (init_values...);` で構造体変数を宣言・初期化する。
- 初期化値はフィールド定義順に対応する。
- 初期化値の一部または全部を省略できる。省略されたフィールドは未初期化。
- 配列フィールドの初期化には配列初期化構文 `[...]` を使用する。

```
let: s@MyStruct;                                 # 全フィールド未初期化 #
let: s@MyStruct (10);                            # number=10, data は未初期化 #
let: s@MyStruct (10, [1,2,3,4,5,6,7,8,9]);      # 全フィールド初期化 #
```

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
struct: MyStruct (number: int, data: int[9]);
# size = 1 + 9 = 10 スロット #
```

構造体変数は内部的に配列として確保される:
- `let: s@MyStruct;` → `let: s[10];` と等価なメモリ確保

### 構造体の制約

- 構造体は値型ではない。代入 (`s1 = s2`) や関数の引数・戻り値としての受け渡しは直接サポートしない。
  - 構造体の参照(`&s`) やフィールド単位の操作で代替する。
- ネストした構造体（フィールドが構造体型）はサポートする。アクセスは `s.inner.field` のようにチェーン可能。

```
struct: Point (x, y);
struct: Rect (top_left @ Point, bottom_right @ Point);

let: r@Rect ((1, 2), (3, 4));
r.top_left.x = 10;   # r[0] = 10 と等価 #
r.bottom_right.y = 20;  # r[3] = 20 と等価 #
```

- 再帰的な構造体定義（自身を含む構造体）は不可（コンパイルエラー）。

## 型仕様 (type_spec) の文法

```bnf
type_spec ::=
    | "int"                        # 整数型
    | "void"                       # 値なし型
    | ident                        # 構造体名（大文字始まり）
    | type_spec "[" integer "]"    # 配列型（将来: 多次元配列）
```

- `int` と `void` は型コンテキスト (`@` の直後) でのみ型として解釈される。
  通常のコンテキストでは識別子として扱われ、後方互換性を維持する。
- 構造体名は大文字で始まる識別子。通常の識別子と名前空間を共有するが、
  大文字制約により名前衝突のリスクは低い。

## 文法の変更点まとめ

```bnf
# 追加トークン
token_at ::= "@"

# 型指定
type_spec ::= "int" | "void" | ident | type_spec "[" integer "]"

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
    | ident ":" type_spec                  # 型を明示: number: int
    | ident "@" type_spec                   # 型を明示 (@形式): data @ Point
    | ident ("[" integer "]")              # 型省略の配列: data[9] (= int[9])
    | ident                                  # 型省略: number (= int)

# グローバル文の拡張
global_stmt ::= ... | struct_decl
stmt ::= ... | struct_decl
```
