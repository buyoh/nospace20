# 組み込みルーチン

旧実装で使用される組み込みユーティリティルーチンの詳細を解説します。

## ルーチンラベル

```cpp
namespace Builder::Alignment {
    const integer LabelUserCodeBegin = 0;

    const integer LabelComparatorZero = 2;
    const integer LabelComparatorZero2 = 3;
    const integer LabelComparatorNegative = 4;
    const integer LabelComparatorNegative2 = 5;
    const integer LabelComparatorAnd = 6;
    const integer LabelComparatorAnd2 = 7;
    const integer LabelComparatorOr = 8;
    const integer LabelComparatorOr2 = 9;
    const integer LabelComparatorOr3 = 10;
}
```

## ゼロ判定ルーチン (LabelComparatorZero)

値がゼロかどうかで異なる値を返します。

### 入力スタック

```
[..., zero_result, nonzero_result, value]
```

### 出力スタック

```
[..., result]
```

- `value == 0` の場合: `result = zero_result`
- `value != 0` の場合: `result = nonzero_result`

### 実装

```
[LabelComparatorZero]       ; スタック: [zero_result, nonzero_result, value]
zerojump [LabelComparatorZero2]  ; value==0 なら分岐
                            ; スタック: [zero_result, nonzero_result]
swap                        ; スタック: [nonzero_result, zero_result]
[LabelComparatorZero2]      ; スタック: [result, discard_value]
discard                     ; スタック: [result]
return
```

### Whitespace コード

```cpp
// [jumped][unjumped][value] zerojump
code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorZero);
code.push(Instruments::Flow::zerojump);
pushInteger(code, Alignment::LabelComparatorZero2);
code.push(Instruments::Stack::swap);
code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorZero2);
code.push(Instruments::Stack::discard);
code.push(Instruments::Flow::retun);
```

### 使用例

**等価比較 (==):**
```
push 1          ; zero の場合の結果
push 0          ; non-zero の場合の結果
[a - b]
call LabelComparatorZero
```

## 負数判定ルーチン (LabelComparatorNegative)

値が負かどうかで異なる値を返します。

### 入力スタック

```
[..., negative_result, nonnegative_result, value]
```

### 出力スタック

```
[..., result]
```

- `value < 0` の場合: `result = negative_result`
- `value >= 0` の場合: `result = nonnegative_result`

### 実装

```
[LabelComparatorNegative]   ; スタック: [neg_result, nonneg_result, value]
negativejump [LabelComparatorNegative2]  ; value<0 なら分岐
                            ; スタック: [neg_result, nonneg_result]
swap                        ; スタック: [nonneg_result, neg_result]
[LabelComparatorNegative2]  ; スタック: [result, discard_value]
discard                     ; スタック: [result]
return
```

### Whitespace コード

```cpp
// [jumped][unjumped][value] negativejump
code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorNegative);
code.push(Instruments::Flow::negativejump);
pushInteger(code, Alignment::LabelComparatorNegative2);
code.push(Instruments::Stack::swap);
code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorNegative2);
code.push(Instruments::Stack::discard);
code.push(Instruments::Flow::retun);
```

### 使用例

**小なり比較 (<):**
```
push 1          ; negative の場合の結果
push 0          ; non-negative の場合の結果
[a - b]
call LabelComparatorNegative
```

## AND ルーチン (LabelComparatorAnd)

論理 AND を計算します。

### 入力スタック

```
[..., value1, value2]
```

### 出力スタック

```
[..., result]
```

- 両方が非ゼロの場合: `result = 1`
- それ以外: `result = 0`

### 実装

```
[LabelComparatorAnd]        ; スタック: [v1, v2]
zerojump [LabelComparatorAnd2]  ; v2==0 なら偽
duplicate                   ; ダミー値（後で discard）
zerojump [LabelComparatorAnd2]  ; v1==0 なら偽
; 両方真
discard                     ; ダミー値を破棄
push 1
return

[LabelComparatorAnd2]
discard                     ; 残りの値を破棄
push 0
return
```

### Whitespace コード

```cpp
code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorAnd);

code.push(Instruments::Flow::zerojump);
pushInteger(code, Alignment::LabelComparatorAnd2);
code.push(Instruments::Stack::duplicate);
code.push(Instruments::Flow::zerojump);
pushInteger(code, Alignment::LabelComparatorAnd2);

code.push(Instruments::Stack::discard);
code.push(Instruments::Stack::push);
pushInteger(code, 1);
code.push(Instruments::Flow::retun);

code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorAnd2);
code.push(Instruments::Stack::discard);
code.push(Instruments::Stack::push);
pushInteger(code, 0);
code.push(Instruments::Flow::retun);
```

## OR ルーチン (LabelComparatorOr)

論理 OR を計算します。

### 入力スタック

```
[..., value1, value2]
```

### 出力スタック

```
[..., result]
```

- どちらかが非ゼロの場合: `result = 1`
- 両方ゼロの場合: `result = 0`

### 実装

```
[LabelComparatorOr]         ; スタック: [v1, v2]
zerojump [LabelComparatorOr2]  ; v2==0 ならチェック続行
; v2!=0 なので真
discard                     ; v1 を破棄
push 1
return

[LabelComparatorOr2]        ; スタック: [v1]
zerojump [LabelComparatorOr3]  ; v1==0 なら偽
; v1!=0 なので真
push 1
return

[LabelComparatorOr3]
; 両方偽
push 0
return
```

### Whitespace コード

```cpp
code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorOr);

code.push(Instruments::Flow::zerojump);
pushInteger(code, Alignment::LabelComparatorOr2);
code.push(Instruments::Stack::discard);
code.push(Instruments::Stack::push);
pushInteger(code, 1);
code.push(Instruments::Flow::retun);

code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorOr2);

code.push(Instruments::Flow::zerojump);
pushInteger(code, Alignment::LabelComparatorOr3);
code.push(Instruments::Stack::push);
pushInteger(code, 1);
code.push(Instruments::Flow::retun);

code.push(Instruments::Flow::label);
pushInteger(code, Alignment::LabelComparatorOr3);
code.push(Instruments::Stack::push);
pushInteger(code, 0);
code.push(Instruments::Flow::retun);
```

## ヘッダー全体構造

`attachEmbeddedHeader` でプログラム開始時に挿入されるコード：

```
; メモリ初期化
push LocalHeapBegin (=2)
push GlobalPtr (=8)
store               ; heap[2] = 8

push LocalHeapEnd (=3)
push (GlobalPtr + global_size)
store               ; heap[3] = 8 + global_size

; ユーザーコードへジャンプ
jump LabelUserCodeBegin (=0)

; 組み込みルーチン群
[LabelComparatorZero のコード]
[LabelComparatorNegative のコード]
[LabelComparatorAnd のコード]
[LabelComparatorOr のコード]

; ユーザーコード開始点
label LabelUserCodeBegin (=0)
```

## フッター構造

`attachEmbeddedFooter` でプログラム終了時に挿入されるコード：

```
call [main のラベル]
exit
```

## 設計上の特徴

### ルーチン使用の利点

- コードサイズ削減：同じ処理を複数回使う場合に効率的
- 保守性：ルーチンを1箇所で変更すれば全体に反映

### ルーチン使用のコスト

- call/return のオーバーヘッド
- 単純な比較でもサブルーチン呼び出しが発生

### 最適化の機会

if 文の条件では、一部の比較演算子に対して直接 zerojump/negativejump を使用する最適化が行われています（[control-flow.md](control-flow.md) 参照）。
