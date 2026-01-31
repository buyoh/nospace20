# 論理演算子

旧実装における論理演算子の Whitespace への変換方法を解説します。

## 演算子 ID 定義

```cpp
namespace Compiler::Embedded::Function {
    const signed IDand = -30;     // &&
    const signed IDor = -31;      // ||
    const signed IDnot = -35;     // !
    const signed IDnotnot = -36;  // !! (最適化用)
}
```

## 論理 NOT (!)

```nospace
!a
```

**生成コード:**
```cpp
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 1);  // zero のときの戻り値
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 0);  // non-zero のときの戻り値
convertExpression(whitesp, exps[0]);

whitesp.push(Instruments::Flow::call);
pushInteger(whitesp, Alignment::LabelComparatorZero);
```

**Whitespace 列:**
```
push 1          ; result if zero (true → 1)
push 0          ; result if non-zero (false → 0)
[a の評価]
call <LabelComparatorZero>
```

**スタック変化:**
- 入力: `[..., 1, 0, a]`
- call後: `[..., (a==0 ? 1 : 0)]`

**真理値表:**
| a | !a |
|---|---|
| 0 | 1 |
| 非0 | 0 |

## 二重否定 (!!)

```nospace
!!a
```

パーサーの最適化により、連続する `!` は `!!` に畳み込まれます。

```cpp
if (typeis<Operation>(*stV)) {
    auto& stvRef = static_cast<Operation&>(*stV);
    if (stvRef.id() == Embedded::Function::IDnot) {
        static_cast<Operation&>(*stV).id() = Embedded::Function::IDnotnot;
        return stV;
    }
    else if (stvRef.id() == Embedded::Function::IDnotnot) {
        static_cast<Operation&>(*stV).id() = Embedded::Function::IDnot;
        return stV;
    }
}
```

**生成コード:**
```cpp
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 0);  // zero のときの戻り値
whitesp.push(Instruments::Stack::push);
pushInteger(whitesp, 1);  // non-zero のときの戻り値
convertExpression(whitesp, exps[0]);

whitesp.push(Instruments::Flow::call);
pushInteger(whitesp, Alignment::LabelComparatorZero);
```

**Whitespace 列:**
```
push 0          ; result if zero
push 1          ; result if non-zero
[a の評価]
call <LabelComparatorZero>
```

**真理値表:**
| a | !!a |
|---|---|
| 0 | 0 |
| 非0 | 1 |

## 論理 AND (&&)

```nospace
a && b
```

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);
convertExpression(whitesp, exps[1]);
whitesp.push(Instruments::Flow::call);
pushInteger(whitesp, Alignment::LabelComparatorAnd);
```

**Whitespace 列:**
```
[a の評価]
[b の評価]
call <LabelComparatorAnd>
```

**スタック変化:**
- 入力: `[..., a, b]`
- call後: `[..., (a && b ? 1 : 0)]`

### AND ルーチンの実装

```
[LabelComparatorAnd]            ; スタック: [v1][v2]
zerojump LabelComparatorAnd2    ; v2==0 なら偽
duplicate                       ; ダミー (discard 対策)
zerojump LabelComparatorAnd2    ; v1==0 なら偽
discard
push 1                          ; 両方真
return

[LabelComparatorAnd2]
discard
push 0                          ; 偽
return
```

**真理値表:**
| a | b | a && b |
|---|---|--------|
| 0 | 0 | 0 |
| 0 | 非0 | 0 |
| 非0 | 0 | 0 |
| 非0 | 非0 | 1 |

**注意:** 短絡評価（ショートサーキット）は行われません。両方のオペランドが常に評価されます。

## 論理 OR (||)

```nospace
a || b
```

**生成コード:**
```cpp
convertExpression(whitesp, exps[0]);
convertExpression(whitesp, exps[1]);
whitesp.push(Instruments::Flow::call);
pushInteger(whitesp, Alignment::LabelComparatorOr);
```

**Whitespace 列:**
```
[a の評価]
[b の評価]
call <LabelComparatorOr>
```

### OR ルーチンの実装

```
[LabelComparatorOr]             ; スタック: [v1][v2]
zerojump LabelComparatorOr2     ; v2==0 ならチェック続行
discard
push 1                          ; v2!=0 なので真
return

[LabelComparatorOr2]            ; スタック: [v1]
zerojump LabelComparatorOr3     ; v1==0 なら偽
push 1                          ; v1!=0 なので真
return

[LabelComparatorOr3]
push 0                          ; 両方偽
return
```

**真理値表:**
| a | b | a \|\| b |
|---|---|--------|
| 0 | 0 | 0 |
| 0 | 非0 | 1 |
| 非0 | 0 | 1 |
| 非0 | 非0 | 1 |

**注意:** 短絡評価は行われません。

## ラベル定数

```cpp
namespace Builder::Alignment {
    const integer LabelComparatorAnd = 6;
    const integer LabelComparatorAnd2 = 7;
    const integer LabelComparatorOr = 8;
    const integer LabelComparatorOr2 = 9;
    const integer LabelComparatorOr3 = 10;
}
```

## 実装上の注意

### 短絡評価なし

旧実装では、論理演算子は短絡評価を行いません。これは以下を意味します：

- `a && b`: `b` は `a` が偽でも評価される
- `a || b`: `b` は `a` が真でも評価される

これは副作用を持つ式で問題となる可能性があります：

```nospace
# 標準的な言語では ptr != 0 が偽なら *ptr は評価されない
# しかし旧実装では *ptr も評価される
ptr != 0 && *ptr > 0
```

### 真偽値の表現

- 偽: `0`
- 真: `非0`（通常は `1`）

論理演算の結果は常に `0` または `1` です。
