# 算術・比較演算子

旧実装における算術演算子と比較演算子の Whitespace への変換方法を解説します。

## 演算子 ID 定義

```cpp
namespace Compiler::Embedded::Function {
    // 算術演算子
    const signed IDaadd = -10;    // +
    const signed IDasub = -11;    // -
    const signed IDamul = -12;    // *
    const signed IDadiv = -13;    // /
    const signed IDamod = -14;    // %
    const signed IDaminus = -15;  // 単項マイナス

    // 比較演算子
    const signed IDequal = -40;      // ==
    const signed IDnotequal = -41;   // !=
    const signed IDless = -42;       // <
    const signed IDlesseq = -43;     // <=
    const signed IDgreater = -44;    // >
    const signed IDgreatereq = -45;  // >=
}
```

## 算術演算子

### 加算 (+)

```nospace
a + b
```

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);  // a
convertExpression(whitesp, exps[1]);  // b
whitesp.push(Instruments::Arithmetic::add);
```

**Whitespace 列:**
```
[a の評価]
[b の評価]
TB SP SP SP     ; add
```

**スタック変化:** `[...] → [..., a] → [..., a, b] → [..., a+b]`

### 減算 (-)

```nospace
a - b
```

**Whitespace 列:**
```
[a の評価]
[b の評価]
TB SP SP TB     ; sub
```

**スタック変化:** `[..., a, b] → [..., a-b]`

### 乗算 (*)

```nospace
a * b
```

**Whitespace 列:**
```
[a の評価]
[b の評価]
TB SP SP LF     ; mul
```

### 除算 (/)

```nospace
a / b
```

**Whitespace 列:**
```
[a の評価]
[b の評価]
TB SP TB SP     ; div
```

### 剰余 (%)

```nospace
a % b
```

**Whitespace 列:**
```
[a の評価]
[b の評価]
TB SP TB TB     ; mod
```

### 単項マイナス (-)

```nospace
-a
```

**生成コード:**
```cpp
whitesp.push(Instruments::Stack::push);
whitesp.push({ Chr::SP, Chr::LF }); // 0
convertExpression(whitesp, exps[0]);
whitesp.push(Instruments::Arithmetic::sub);
```

**Whitespace 列:**
```
SP SP SP LF     ; push 0
[a の評価]
TB SP SP TB     ; sub  (0 - a)
```

**最適化:** 定数値の場合はコンパイル時に符号反転

```cpp
if (typeis<FactorValue>(*stV)) {
    static_cast<FactorValue&>(*stV).get() *= -1;
    return stV;
}
```

## 比較演算子

比較演算子は組み込みルーチンを呼び出して実装されています。

### 等価比較 (==)

```nospace
a == b
```

**生成コード:**
```cpp
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 1);  // zero のときの戻り値
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 0);  // non-zero のときの戻り値
convertExpression(whitesp, exps[0]);
convertExpression(whitesp, exps[1]);
whitesp.push(Instruments::Arithmetic::sub);  // a - b

whitesp.push(Instruments::Flow::call);
pushInteger(whitesp, Alignment::LabelComparatorZero);
```

**Whitespace 列:**
```
push 1          ; result if zero
push 0          ; result if non-zero
[a の評価]
[b の評価]
sub             ; a - b
call <LabelComparatorZero>
```

**スタック変化:** 
- 入力: `[..., 1, 0, a, b]`
- sub後: `[..., 1, 0, a-b]`
- call後: `[..., (a==b ? 1 : 0)]`

### 非等価比較 (!=)

```nospace
a != b
```

`==` と同様だが、戻り値が逆転：

```
push 0          ; result if zero
push 1          ; result if non-zero
[a - b の計算]
call <LabelComparatorZero>
```

### 小なり (<)

```nospace
a < b
```

**生成コード:**
```cpp
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 1);  // negative のときの戻り値
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 0);  // non-negative のときの戻り値
convertExpression(whitesp, exps[0]);
convertExpression(whitesp, exps[1]);
whitesp.push(Instruments::Arithmetic::sub);

whitesp.push(Instruments::Flow::call);
pushInteger(whitesp, Alignment::LabelComparatorNegative);
```

**Whitespace 列:**
```
push 1          ; result if negative
push 0          ; result if non-negative
[a の評価]
[b の評価]
sub             ; a - b (a < b ⟺ a-b < 0)
call <LabelComparatorNegative>
```

### 大なりイコール (>=)

`<` と同様だが、戻り値が逆転：

```
push 0          ; result if negative
push 1          ; result if non-negative
[a - b の計算]
call <LabelComparatorNegative>
```

### 大なり (>)

```nospace
a > b
```

オペランドの順序を入れ替えて実装：

```cpp
convertExpression(whitesp, exps[1]);  // b を先に
convertExpression(whitesp, exps[0]);  // a を後に
whitesp.push(Instruments::Arithmetic::sub);  // b - a
```

**Whitespace 列:**
```
push 1          ; result if negative
push 0          ; result if non-negative
[b の評価]
[a の評価]
sub             ; b - a (a > b ⟺ b-a < 0)
call <LabelComparatorNegative>
```

### 小なりイコール (<=)

`>` と同様だが、戻り値が逆転：

```
push 0          ; result if negative
push 1          ; result if non-negative
[b - a の計算]
call <LabelComparatorNegative>
```

## 比較演算子ルーチン

詳細は [builtin-routines.md](builtin-routines.md) を参照。

- `LabelComparatorZero`: 値がゼロかどうかで分岐
- `LabelComparatorNegative`: 値が負かどうかで分岐

## 演算子の優先順位と結合性

パーサーが処理する順序（優先順位低→高）：

1. `=`, `+=`, `-=`, `*=`, `/=`, `%=` (右結合)
2. `&&`, `||` (左結合)
3. `==`, `!=`, `<`, `<=`, `>`, `>=` (左結合)
4. `+`, `-` (左結合)
5. `*`, `/`, `%` (左結合)
6. 単項 `-`, `!`, `*` (前置)
7. `[]` (後置)
