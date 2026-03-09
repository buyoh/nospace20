# 型システム: 構造体のメモリレイアウト

## 概要

nospace は Whitespace をターゲットとするため、メモリモデルはフラットなアドレス空間（スタック + ヒープ）である。構造体はこのフラットなメモリ上に連続して配置される。

## レイアウト規則

### 基本

- フィールドは定義順にパディングなしで配置される。
- 各 int フィールドは 1 スロット。
- 各 int[N] フィールドは N スロット。
- ネストした構造体フィールドは、その構造体の合計サイズ分のスロットを消費する。

### 例

```
struct: Point (x, y);              # 型省略 → 各フィールド int #
# x: offset 0, size 1
# y: offset 1, size 1
# total_size: 2

struct: Line (start @ Point, end @ Point);  # 構造体フィールド #
# start: offset 0, size 2  (Point の total_size)
# end:   offset 2, size 2
# total_size: 4

struct: MyStruct (number, data[9]);  # 型省略 + 配列 #
# number: offset 0, size 1
# data:   offset 1, size 9
# total_size: 10

# 明示的な型指定でも同じ結果 #
struct: MyStruct (number: int, data: int[9]);
```

## 変数確保

### ローカル変数

```
let: s@MyStruct (10, [1,2,3,4,5,6,7,8,9]);
```

内部変換:
```
let: s[10]([10, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
```

- 構造体変数は配列として確保される。
- 初期化値は構造体レイアウトに沿って展開される。
- `s` 単体は `s[0]` と同義（配列の規則に従う）。

### 初期化の展開

構造体の初期化 `(val1, [val2, val3, ...])` は、フィールドのオフセット順にフラットに展開される:

```
struct: MyStruct (number: int, data: int[9]);
let: s@MyStruct (10, [1,2,3,4,5,6,7,8,9]);

# 内部的に:
# slot[0] = 10     (number)
# slot[1] = 1      (data[0])
# slot[2] = 2      (data[1])
# ...
# slot[9] = 9      (data[8])
```

部分初期化:
```
let: s@MyStruct (10);
# slot[0] = 10     (number)
# slot[1..9] は未初期化
```

### ネストした構造体の初期化

```
struct: Point (x: int, y: int);
struct: Line (start: Point, end: Point);

let: line@Line ((1, 2), (3, 4));
# slot[0] = 1  (start.x)
# slot[1] = 2  (start.y)
# slot[2] = 3  (end.x)
# slot[3] = 4  (end.y)
```

## フィールドアクセスのコード生成

### 読み取り

```
s.number     → *(addr_of_s + 0)   → s[0]
s.data[i]    → *(addr_of_s + 1 + i)  → s[1 + i]  ※ ただし s[1+i] の形式ではなく、アドレス計算

line.start.x → *(addr_of_line + 0 + 0)  → line[0]
line.end.y   → *(addr_of_line + 2 + 1)  → line[3]
```

### 書き込み

```
s.number = 10;     → *(addr_of_s + 0) = 10;
s.data[i] = 42;   → *(addr_of_s + 1 + i) = 42;
```

### 参照取得

```
&s.number    → addr_of_s + 0
&s.data      → addr_of_s + 1
&s.data[i]   → addr_of_s + 1 + i
```

## 構造体ビュー (`@ StructName`)

任意の配列やポインタ先を構造体として再解釈する仕組み。

```
let: data[10];
(data @ MyStruct).number = 10;
```

これは以下と等価:
```
let: data[10];
*((&data) + 0) = 10;  # data[0] = 10 #
```

### 実装

`@ StructName` は型情報を付与するだけで、メモリレイアウトは変わらない。
フィールドアクセス時に、注釈された構造体の定義に基づいてオフセットを計算する。

### 制約

- 対象の配列サイズが構造体の `total_size` 以上であることをコンパイル時に検証。
- ポインタ (`*ptr @ MyStruct`) に対しては動的なため、コンパイル時の検証はできない。

## ヒープ上の構造体

`__alloc` で確保したメモリにも構造体ビューを適用できる:

```
struct: Node (value: int, next: int);

let: node;
node = __alloc(2);  # Node のサイズ分確保 #
(* node @ Node).value = 42;
(* node @ Node).next = 0;
```

これは以下と等価:
```
let: node;
node = __alloc(2);
*(node + 0) = 42;
*(node + 1) = 0;
```

## サイズ計算アルゴリズム

```
compute_size(type_spec):
    match type_spec:
        Int → 1
        Void → 0  (エラー: void フィールドは不可)
        Named(name) → struct_definitions[name].total_size
        Array(inner, n) → compute_size(inner) * n

compute_struct_layout(fields):
    offset = 0
    for (name, type_spec) in fields:
        size = compute_size(type_spec)
        field_info = { name, type_spec, offset, size }
        offset += size
    total_size = offset
```

## Whitespace ターゲットでの考慮事項

- Whitespace のスタックとヒープは同じアドレス空間を共有する。
- 構造体のフィールドアクセスは、アドレス計算 + `store`/`retrieve` 命令に変換される。
- オフセットがコンパイル時定数なので、`push offset; add; retrieve` の形で効率的なコード生成が可能。
