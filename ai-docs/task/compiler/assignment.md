# 代入演算子

旧実装における代入演算子の Whitespace への変換方法を解説します。

## 演算子 ID 定義

```cpp
namespace Compiler::Embedded::Function {
    const signed IDassign = -60;     // =
    const signed IDassignadd = -61;  // +=
    const signed IDassignsub = -62;  // -=
    const signed IDassignmul = -63;  // *=
    const signed IDassigndiv = -64;  // /=
    const signed IDassignmod = -65;  // %=
    const signed IDdereference = -70; // *ptr
    const signed IDindexer = -75;     // arr[i]
}
```

## 単純代入 (=)

### 変数への代入

```nospace
x = value
```

**生成コード:**
```cpp
// 1. 左辺のアドレスを計算
convertCalculateLocalVariablePtr(whitesp, var);

// 2. アドレスを複製（後で値を読み取るため）
whitesp.push(Instruments::Stack::duplicate);

// 3. 右辺の値を計算
convertExpression(whitesp, exps[1]);

// 4. ヒープに格納
whitesp.push(Instruments::Heap::store);

// 5. 格納した値を取り出す（式の戻り値として）
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
[x のアドレス計算]
duplicate           ; アドレスを複製
[value の評価]
store               ; heap[addr] = value
retrieve            ; push(heap[addr])
```

**スタック変化:**
```
[...]
[..., addr]         ; アドレス計算
[..., addr, addr]   ; duplicate
[..., addr, addr, value] ; value 評価
[..., addr]         ; store
[..., value]        ; retrieve (式の結果)
```

### デリファレンスへの代入

```nospace
*ptr = value
```

**生成コード:**
```cpp
if (typeis<Operation>(exps[0])) {
    const auto& dref = static_cast<const Operation&>(exps[0]);
    if (dref.id() == Embedded::Function::IDdereference) {
        convertExpression(whitesp, dref[0]);  // ptr の値を評価
    }
}
// 以降は変数代入と同じ
```

**Whitespace 列:**
```
[ptr の評価]        ; ptr の値がアドレスとなる
duplicate
[value の評価]
store
retrieve
```

### 配列要素への代入

```nospace
arr[i] = value
```

**生成コード:**
```cpp
if (dref.id() == Embedded::Function::IDindexer) {
    convertExpression(whitesp, dref[0]);  // arr のアドレス
    convertExpression(whitesp, dref[1]);  // i の値
    whitesp.push(Instruments::Arithmetic::add);  // arr + i
}
// 以降は変数代入と同じ
```

**Whitespace 列:**
```
[arr のアドレス]
[i の評価]
add                 ; arr + i
duplicate
[value の評価]
store
retrieve
```

## 複合代入演算子

### 加算代入 (+=)

```nospace
x += value
```

**生成コード:**
```cpp
// アドレスを計算
convertCalculateLocalVariablePtr(whitesp, var);

// 3回複製：store用、演算用、retrieve用
whitesp.push(Instruments::Stack::duplicate);
whitesp.push(Instruments::Stack::duplicate);

// 現在値を取得
whitesp.push(Instruments::Heap::retrieve);

// 右辺を評価して演算
convertExpression(whitesp, exps[1]);
whitesp.push(Instruments::Arithmetic::add);

// 格納
whitesp.push(Instruments::Heap::store);

// 結果を取り出す
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
[x のアドレス]
duplicate           ; [addr, addr]
duplicate           ; [addr, addr, addr]
retrieve            ; [addr, addr, x]
[value の評価]      ; [addr, addr, x, value]
add                 ; [addr, addr, x+value]
store               ; [addr]  heap[addr] = x+value
retrieve            ; [x+value]
```

### 減算代入 (-=)

`+=` と同様で、`add` の代わりに `sub` を使用。

### 乗算代入 (*=)

`+=` と同様で、`add` の代わりに `mul` を使用。

### 除算代入 (/=)

`+=` と同様で、`add` の代わりに `div` を使用。

### 剰余代入 (%=)

`+=` と同様で、`add` の代わりに `mod` を使用。

## デリファレンス (*)

```nospace
*ptr
```

ポインタが指す値を取得します。

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
[ptr の評価]        ; スタックにアドレスが積まれる
retrieve            ; heap[ptr]
```

## インデクサ ([])

```nospace
arr[i]
```

配列の要素を取得します。

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);  // arr のアドレス
convertExpression(whitesp, exps[1]);  // i
whitesp.push(Instruments::Arithmetic::add);
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
[arr のアドレス]
[i の評価]
add                 ; arr + i
retrieve            ; heap[arr + i]
```

## アドレス取得 (&)

```nospace
&x
```

変数のアドレスを取得します。

**処理:** パーサーで `FactorAddress` に変換され、アドレス計算のみが行われます（retrieve なし）。

**Whitespace 列:**
```
[x のアドレス計算]  ; 値そのものがアドレス
; retrieve は行わない
```

## 結合性

代入演算子は**右結合**です：

```nospace
a = b = c
```

は以下のように解釈されます：

```nospace
a = (b = c)
```

パーサーでの処理：

```cpp
// 右結合
auto commonProcedure = [&](Operation* new_ex) {
    new_ex->args(0) = move(*curr);
    curr->reset(new_ex);
    curr = &(*new_ex).args(1);  // 右側に潜っていく
    stream.get();
};
```

## 戻り値

すべての代入演算子は、代入された値を戻り値として返します。これにより連鎖代入が可能になります：

```nospace
a = b = c = 10;
# a, b, c すべてに 10 が代入される
```
