# 制御構造

旧実装における制御構造（if/elsif/else、while）の Whitespace への変換方法を解説します。

## ラベル管理

制御構造にはジャンプ先のラベルが必要です。各制御構造は2つのラベルを確保します。

```cpp
auto label = nameTable->reserveLabelAddr(2);  // label と label+1 を確保
```

### ラベルのオフセット

ユーザーコードのラベルには `LabelOffset` が加算されます：

```cpp
const integer LabelOffset = 16;

inline integer solveLabel(integer labelId) {
    return labelId + Alignment::LabelOffset;
}
```

これにより、ラベル 0-15 は組み込みルーチン用に予約されます。

## while 文

### 構文

```nospace
while(condition) {
    body
}
```

### 構造

```
[label]         ; ループ先頭
  条件評価
  zerojump [label+1]  ; 条件が偽ならループ終了
  
  本体
  
  jump [label]  ; ループ先頭へ戻る
[label+1]       ; ループ終了
```

### 生成コード

```cpp
WhiteSpace& convertWhile(WhiteSpace& whitesp, const StatementWhile& whilestat) {
    integer label = solveLabel(whilestat.label);

    // ループ先頭ラベル
    whitesp.push(Instruments::Flow::label);
    pushInteger(whitesp, label);

    // 条件評価
    convertExpression(whitesp, *(whilestat.cond));
    
    // 条件が0ならループ終了へジャンプ
    whitesp.push(Instruments::Flow::zerojump);
    pushInteger(whitesp, label + 1);

    // 本体
    convertOpenScope(whitesp, whilestat);

    // ループ先頭へ戻る
    whitesp.push(Instruments::Flow::jump);
    pushInteger(whitesp, label);
    
    // ループ終了ラベル
    whitesp.push(Instruments::Flow::label);
    pushInteger(whitesp, label + 1);

    return whitesp;
}
```

### Whitespace 列

```
label [L]           ; LF SP SP [L]
[条件の評価]
zerojump [L+1]      ; LF TB SP [L+1]
[本体のコード]
jump [L]            ; LF SP LF [L]
label [L+1]         ; LF SP SP [L+1]
```

### 例

```nospace
let: i(0);
while(i < 10) {
    i += 1;
}
```

**生成される構造:**
```
label 16            ; ループ先頭
; i < 10 の評価
push 1              ; negative の戻り値
push 0              ; non-negative の戻り値
[i の読み取り]
push 10
sub                 ; i - 10
call 4              ; LabelComparatorNegative
zerojump 17         ; 条件が偽なら終了
; i += 1
[i のアドレス]
dup
dup
retrieve
push 1
add
store
retrieve
discard             ; 式の値を捨てる
jump 16             ; ループ先頭へ
label 17            ; ループ終了
```

## if 文

### 構文

```nospace
if(condition) {
    body
}
```

### 構造（if のみ）

```
[label]             ; if 先頭（使用されない場合あり）
  条件評価
  zerojump [label+1]  ; 条件が偽なら if 終了
  
  本体
  
[label+1]           ; if 終了
```

### 生成コード

```cpp
WhiteSpace& convertIf(WhiteSpace& whitesp, const StatementIf& ifstat) {
    integer label = solveLabel(ifstat.label);

    // if 先頭ラベル（elsif の連鎖で使用）
    whitesp.push(Instruments::Flow::label);
    pushInteger(whitesp, label);

    if (ifstat.cond) {
        convertIf_condition(whitesp, ifstat, label);
    }

    // 本体
    convertOpenScope(whitesp, ifstat);

    if (ifstat.elsif) {
        // else/elsif がある場合、最後のラベルへジャンプ
        whitesp.push(Instruments::Flow::jump);
        pushInteger(whitesp, solveLabel(ifstat.getLabelLast()) + 1);
    }

    // if 終了ラベル
    whitesp.push(Instruments::Flow::label);
    pushInteger(whitesp, label + 1);

    // elsif/else の処理
    if (ifstat.elsif)
        convertIf(whitesp, *(ifstat.elsif));

    return whitesp;
}
```

### 条件評価の最適化

特定の比較演算子に対して最適化が行われます：

```cpp
WhiteSpace& convertIf_condition(WhiteSpace& whitesp, const StatementIf& ifstat, integer label) {
    const Expression& expr = *(ifstat.cond);

    if (typeis<Operation>(expr)) {
        const Operation& op = static_cast<const Operation&>(expr);
        switch (op.id()) {
        case Embedded::Function::IDnotequal: {
            // a != b は (a-b)==0 で分岐
            convertExpression(whitesp, op[0]);
            convertExpression(whitesp, op[1]);
            whitesp.push(Instruments::Arithmetic::sub);
            whitesp.push(Instruments::Flow::zerojump);
            pushInteger(whitesp, label + 1);
            return whitesp;
        }
        case Embedded::Function::IDgreatereq: {
            // a >= b は (a-b)<0 で分岐
            convertExpression(whitesp, op[0]);
            convertExpression(whitesp, op[1]);
            whitesp.push(Instruments::Arithmetic::sub);
            whitesp.push(Instruments::Flow::negativejump);
            pushInteger(whitesp, label + 1);
            return whitesp;
        }
        case Embedded::Function::IDlesseq: {
            // a <= b は (b-a)<0 で分岐
            convertExpression(whitesp, op[1]);
            convertExpression(whitesp, op[0]);
            whitesp.push(Instruments::Arithmetic::sub);
            whitesp.push(Instruments::Flow::negativejump);
            pushInteger(whitesp, label + 1);
            return whitesp;
        }
        }
    }
    
    // 一般的な場合
    convertExpression(whitesp, expr);
    whitesp.push(Instruments::Flow::zerojump);
    pushInteger(whitesp, label + 1);
    return whitesp;
}
```

## if-else 文

### 構文

```nospace
if(condition) {
    body_if
} else {
    body_else
}
```

### 構造

```
[label0@if]
  条件評価
  zerojump [label1@if]    ; 条件が偽なら else へ
  
  body_if
  
  jump [label1@else]      ; 全体の終わりへ
[label1@if]
[label0@else]
  body_else
[label1@else]             ; 全体の終了
```

### Whitespace 列

```
label [L_if]        ; LF SP SP [L_if]
[条件の評価]
zerojump [L_if+1]   ; LF TB SP [L_if+1]
[body_if のコード]
jump [L_else+1]     ; LF SP LF [L_else+1]
label [L_if+1]      ; LF SP SP [L_if+1]
label [L_else]      ; LF SP SP [L_else]
[body_else のコード]
label [L_else+1]    ; LF SP SP [L_else+1]
```

## if-elsif-else 文

### 構文

```nospace
if(cond1) {
    body1
} elsif(cond2) {
    body2
} else {
    body3
}
```

### 構造

```
[label0@if]
  cond1 評価
  zerojump [label1@if]
  body1
  jump [label1@else]      ; 最後の else の終わりへ
[label1@if]
[label0@elsif]
  cond2 評価
  zerojump [label1@elsif]
  body2
  jump [label1@else]      ; 最後の else の終わりへ
[label1@elsif]
[label0@else]
  body3
[label1@else]
```

### 最後のラベル取得

連鎖した if の最後のラベルを取得するメソッド：

```cpp
integer getLabelLast() const {
    return elsif ? elsif->getLabelLast() : label;
}
```

## 旧実装のコメント

ソースコード末尾に記載されているラベル設計メモ：

```
// if only
// [label0@if]
// zj label1@if
// block
// [label1@if]

// ifelse
// [label0@if]
// zj label1@if
// block
// j label1@else
// [label1@if]
// [label0@else]
// block
// [label1@else]

// elsif
// [label0@if]
// zj label1@if
// block
// j label1@else
// [label1@if]
// [label0@elsif]
// zj label1@elsif
// block
// j label1@else
// [label1@elsif]
// [label0@else]
// block
// [label1@else]
```
