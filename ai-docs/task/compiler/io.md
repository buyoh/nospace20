# I/O 関数

旧実装における入出力関数の Whitespace への変換方法を解説します。

## 組み込み I/O 関数

```cpp
reservedNameTable.defineEmbeddedFunction("__puti", Embedded::Function::IDputi, 1);
reservedNameTable.defineEmbeddedFunction("__putc", Embedded::Function::IDputc, 1);
reservedNameTable.defineEmbeddedFunction("__geti", Embedded::Function::IDgeti, 0);
reservedNameTable.defineEmbeddedFunction("__getc", Embedded::Function::IDgetc, 0);
reservedNameTable.defineEmbeddedFunction("__getiv", Embedded::Function::IDgetiv, 1);
reservedNameTable.defineEmbeddedFunction("__getcv", Embedded::Function::IDgetcv, 1);
```

## 出力関数

### __puti(n) - 整数出力

整数を10進数で出力します。

```nospace
__puti(42);
```

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);
whitesp.push(Instruments::Stack::duplicate);
whitesp.push(Instruments::IO::putnumber);
```

**Whitespace 列:**
```
[引数の評価]        ; スタック: [n]
duplicate           ; SP LF SP, スタック: [n, n]
putnumber           ; TB LF SP TB, n を出力, スタック: [n]
```

**スタック変化:** `[...] → [..., n] → [..., n, n] → [..., n]`

**戻り値:** 出力した値がそのまま返されます。

### __putc(c) - 文字出力

文字（ASCII コード）を出力します。

```nospace
__putc('A');
```

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);
whitesp.push(Instruments::Stack::duplicate);
whitesp.push(Instruments::IO::putchar);
```

**Whitespace 列:**
```
[引数の評価]        ; スタック: [c]
duplicate           ; SP LF SP
putchar             ; TB LF SP SP, 文字を出力
```

**戻り値:** 出力した文字コードがそのまま返されます。

## 入力関数

### __geti() - 整数入力

整数を読み込みます。

```nospace
let: n(__geti());
```

**生成コード:**
```cpp
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, Alignment::TempPtr);
whitesp.push(Instruments::Stack::duplicate);
whitesp.push(Instruments::IO::getnumber);
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
push TempPtr (=4)   ; SP SP [4] LF
duplicate           ; SP LF SP, スタック: [4, 4]
getnumber           ; TB LF TB TB, heap[4] に入力値を格納
retrieve            ; TB TB TB, スタック: [heap[4]]
```

**処理の流れ:**
1. 一時アドレス (TempPtr=4) をプッシュ
2. 複製して2つのアドレスをスタックに
3. `getnumber` で入力値を heap[4] に格納（アドレスを1つ消費）
4. `retrieve` で heap[4] の値をスタックに取り出す

**戻り値:** 入力された整数値

### __getc() - 文字入力

文字を読み込みます。

```nospace
let: c(__getc());
```

**生成コード:**
```cpp
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, Alignment::TempPtr);
whitesp.push(Instruments::Stack::duplicate);
whitesp.push(Instruments::IO::getchar);
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
push TempPtr (=4)   ; SP SP [4] LF
duplicate           ; SP LF SP
getchar             ; TB LF TB SP, heap[4] に入力文字を格納
retrieve            ; TB TB TB
```

**戻り値:** 入力された文字のASCIIコード

### __getiv(addr) - アドレス指定整数入力

指定したアドレスに整数を読み込みます。

```nospace
let: arr[10];
__getiv(&arr[0]);
```

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);  // アドレス
whitesp.push(Instruments::Stack::duplicate);
whitesp.push(Instruments::IO::getnumber);
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
[アドレスの評価]    ; スタック: [addr]
duplicate           ; SP LF SP, スタック: [addr, addr]
getnumber           ; TB LF TB TB, heap[addr] に入力値を格納
retrieve            ; TB TB TB, スタック: [heap[addr]]
```

**戻り値:** 入力された整数値

### __getcv(addr) - アドレス指定文字入力

指定したアドレスに文字を読み込みます。

```nospace
let: buf[100];
__getcv(&buf[0]);
```

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);  // アドレス
whitesp.push(Instruments::Stack::duplicate);
whitesp.push(Instruments::IO::getchar);
whitesp.push(Instruments::Heap::retrieve);
```

**Whitespace 列:**
```
[アドレスの評価]    ; スタック: [addr]
duplicate           ; SP LF SP
getchar             ; TB LF TB SP, heap[addr] に入力文字を格納
retrieve            ; TB TB TB
```

**戻り値:** 入力された文字のASCIIコード

## Whitespace I/O 命令詳細

### putnumber (TB LF SP TB)

- スタックからポップした値を10進数として出力
- スタック: `[..., n] → [...]`

### putchar (TB LF SP SP)

- スタックからポップした値を文字として出力
- スタック: `[..., c] → [...]`

### getnumber (TB LF TB TB)

- 10進数を読み込み、スタックトップのアドレスに格納
- スタック: `[..., addr] → [...]`
- ヒープ: `heap[addr] = 入力値`

### getchar (TB LF TB SP)

- 文字を読み込み、スタックトップのアドレスに格納
- スタック: `[..., addr] → [...]`
- ヒープ: `heap[addr] = 入力文字`

## 注意事項

### 戻り値の仕様

すべての I/O 関数は入出力した値を戻り値として返します。これにより式の中で使用できます：

```nospace
# 入力した値を2倍して出力
__puti(__geti() * 2);
```

### 一時領域の使用

`__geti()` と `__getc()` は一時領域 `TempPtr` (アドレス 4) を使用します。この領域は他の操作でも使用される可能性があるため、入力値はすぐに変数に格納するべきです。

```nospace
# 推奨
let: n(__geti());

# 非推奨（複数の __geti を1式で使う）
# let: sum(__geti() + __geti());  # 動作するが一時領域の競合に注意
```

### v 系関数の用途

`__getiv` と `__getcv` は配列への直接入力に便利です：

```nospace
let: arr[10];
let: i(0);
while(i < 10) {
    __getiv(&arr[i]);
    i += 1;
}
```
