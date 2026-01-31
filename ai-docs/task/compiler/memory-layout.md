# メモリレイアウトと管理

旧実装における Whitespace のヒープメモリ管理方式を解説します。

## メモリレイアウト

### 予約アドレス

```cpp
namespace Builder::Alignment {
    const integer LocalHeapBegin = 2;   // ローカルヒープ開始位置を格納
    const integer LocalHeapEnd = 3;     // ローカルヒープ終了位置を格納
    const integer TempPtr = 4;          // 一時領域 (4-7)
    const integer GlobalPtr = 8;        // グローバル変数領域の開始位置
}
```

### メモリマップ

```
アドレス 0: 未使用（ゼロ push の最適化用）
アドレス 1: 予約
アドレス 2: LocalHeapBegin（現在のローカルスコープ開始位置）
アドレス 3: LocalHeapEnd（現在のローカルスコープ終了位置）
アドレス 4-7: 一時作業領域 (TempPtr, TempPtr+1, ...)
アドレス 8+: グローバル変数領域

[ローカル変数領域はグローバル領域の後ろに動的に確保]
```

## 変数アドレス解決

### グローバル変数

グローバル変数は静的アドレスを持ちます：

```
実効アドレス = GlobalPtr + 変数オフセット
             = 8 + offset
```

### ローカル変数

ローカル変数はスタックフレームベースの相対アドレスを使用します：

```
実効アドレス = heap[LocalHeapBegin] + 変数オフセット
```

### アドレス計算コード

```cpp
WhiteSpace& convertCalculateLocalVariablePtr(WhiteSpace& whitesp, const FactorVariable& var) {
    if (var.scope() > 0) {
        // ローカル変数
        whitesp.push(Instruments::Stack::push);
        pushInteger(whitesp, var.get());  // オフセット

        whitesp.push(Instruments::Stack::push);
        pushInteger(whitesp, Alignment::LocalHeapBegin);
        whitesp.push(Instruments::Heap::retrieve);  // 現在のローカル開始位置

        whitesp.push(Instruments::Arithmetic::add);
    }
    else {
        // グローバル変数
        whitesp.push(Instruments::Stack::push);
        pushInteger(whitesp, var.get() + Alignment::GlobalPtr);
    }
    return whitesp;
}
```

### 生成される Whitespace 列

**グローバル変数 (offset=0):**
```
push 8          ; GlobalPtr + offset
                ; → SP SP [8のエンコード] LF
```

**ローカル変数 (offset=2):**
```
push 2          ; offset
push 2          ; LocalHeapBegin
retrieve        ; heap[LocalHeapBegin]
add             ; offset + heap[LocalHeapBegin]
```

## スタックフレーム管理

### 関数呼び出し時の割り当て (convertLocalAllocate)

関数に入る際、新しいローカル変数領域を確保します：

```
1. 現在の local_begin をスタックに退避
2. local_begin := local_end（新しいスコープの開始）
3. local_end := local_begin + scopesize（新しい終了位置）
```

**生成コード:**

```cpp
// 1. local_begin を退避
push LocalHeapBegin (=2)
retrieve                    // heap[2] をスタックに

// 2. local_end を取得し、local_begin に設定
push LocalHeapEnd (=3)
duplicate
retrieve                    // heap[3]
push LocalHeapBegin (=2)
copy 1                      // local_end の値をコピー
store                       // heap[2] = heap[3]

// 3. local_end を更新
push <scopesize>
add
store                       // heap[3] = old_local_end + scopesize
```

### 関数からの復帰時の解放 (convertLocalDeallocate)

```
1. local_end := local_begin
2. local_begin := スタックから復元
```

**生成コード:**

```cpp
// 1. local_end := local_begin
// convertCopy(LocalHeapEnd, LocalHeapBegin)
push LocalHeapEnd (=3)
push LocalHeapBegin (=2)
retrieve
store

// 2. local_begin をスタックから復元
push LocalHeapBegin (=2)
swap                        // [saved_begin, 2] → [2, saved_begin]
store                       // heap[2] = saved_begin
```

## メモリコピー操作

`convertCopy` はヒープ間のコピーを行います：

```cpp
// *destPtr = *fromPtr
WhiteSpace& convertCopy(WhiteSpace& whitesp, integer destPtr, integer fromPtr) {
    whitesp.push(Instruments::Stack::push);
    pushInteger(whitesp, destPtr);
    whitesp.push(Instruments::Stack::push);
    pushInteger(whitesp, fromPtr);
    whitesp.push(Instruments::Heap::retrieve);
    whitesp.push(Instruments::Heap::store);
    return whitesp;
}
```

**生成される Whitespace:**
```
push <destPtr>
push <fromPtr>
retrieve        ; heap[fromPtr]
store           ; heap[destPtr] = heap[fromPtr]
```

## 初期化処理 (attachEmbeddedHeader)

プログラム開始時のメモリ初期化：

```cpp
// LocalHeapBegin := GlobalPtr (グローバル変数の後ろから開始)
push LocalHeapBegin (=2)
push GlobalPtr (=8)
store

// LocalHeapEnd := GlobalPtr + グローバル変数サイズ
push LocalHeapEnd (=3)
push (GlobalPtr + globalScope.localHeapSize)
store
```

## スタックとヒープの使い分け

| 用途 | 使用領域 |
|------|----------|
| 式の評価 | スタック |
| 関数引数 | スタック → ヒープ (ローカル変数へコピー) |
| 関数戻り値 | スタック |
| 変数格納 | ヒープ |
| スコープ情報退避 | スタック |
