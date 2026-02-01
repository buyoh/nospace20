# 関数呼び出し

旧実装における関数定義と呼び出しの Whitespace への変換方法を解説します。

## 関数定義

### 構文

```nospace
func: name(arg1, arg2) {
    body
}
```

### 構造

```
jump [label+1]      ; 関数本体をスキップ
[label]             ; 関数エントリポイント
  ; ローカル変数領域確保
  ; 引数をローカル変数にコピー
  
  body
  
  ; ローカル変数領域解放
  push 0            ; デフォルト戻り値
  return
[label+1]           ; 関数定義の終わり
```

### 生成コード

```cpp
WhiteSpace& convertFunction(WhiteSpace& whitesp, const StatementFunction& func) {
    integer label = solveLabel(func.funcLabel);

    // 関数本体をスキップするジャンプ
    whitesp.push(Instruments::Flow::jump);
    pushInteger(whitesp, label + 1);

    // 関数エントリポイント
    whitesp.push(Instruments::Flow::label);
    pushInteger(whitesp, label);

    // ローカル変数領域確保
    convertLocalAllocate(whitesp, func);

    // 引数の処理
    if (!func.argAddrs.empty()) {
        // 戻り先アドレスを一時退避
        whitesp.push(Instruments::Stack::push);
        pushInteger(whitesp, Alignment::TempPtr);
        whitesp.push(Instruments::Stack::swap);
        whitesp.push(Instruments::Heap::store);

        // 引数をローカル変数にコピー
        int bk = int(func.argAddrs.size()) - 1;
        
        // 基準アドレスを計算して保存
        whitesp.push(Instruments::Stack::push);
        pushInteger(whitesp, Alignment::TempPtr + 1);
        convertCalculateLocalVariablePtr(whitesp, func.argAddrs[bk]);
        whitesp.push(Instruments::Heap::store);
        
        for (int i = bk; 0 <= i; --i) {
            // 基準アドレスを取り出し
            whitesp.push(Instruments::Stack::push);
            pushInteger(whitesp, Alignment::TempPtr + 1);
            whitesp.push(Instruments::Heap::retrieve);
            
            // オフセットを加算
            if (func.argAddrs[i] != func.argAddrs[bk]) {
                whitesp.push(Instruments::Stack::push);
                pushInteger(whitesp, func.argAddrs[i] - func.argAddrs[bk]);
                whitesp.push(Instruments::Arithmetic::add);
            }
            
            // 引数値をローカル変数に格納
            whitesp.push(Instruments::Stack::swap);
            whitesp.push(Instruments::Heap::store);
        }

        // 戻り先アドレスを復元
        whitesp.push(Instruments::Stack::push);
        pushInteger(whitesp, Alignment::TempPtr);
        whitesp.push(Instruments::Heap::retrieve);
    }

    // 関数本体
    convertScope(whitesp, dynamic_cast<const StatementScope&>(func));

    // ローカル変数領域解放
    convertLocalDeallocate(whitesp);

    // デフォルト戻り値
    whitesp.push(Instruments::Stack::push);
    pushInteger(whitesp, 0);

    // 呼び出し元へ戻る
    whitesp.push(Instruments::Flow::retun);

    // 関数定義終了ラベル
    whitesp.push(Instruments::Flow::label);
    pushInteger(whitesp, label + 1);
    
    return whitesp;
}
```

## 関数呼び出し

### 構文

```nospace
name(arg1, arg2)
```

### 呼び出し時のスタック

```
呼び出し前: [...]
引数プッシュ: [..., arg1, arg2]
call 実行: [..., arg1, arg2, 戻り先]
関数内: [..., arg1, arg2, 戻り先, 旧local_begin]
return 後: [..., 戻り値]
```

### 生成コード

```cpp
WhiteSpace& convertExpression(WhiteSpace& whitesp, const Expression& exps) {
    if (typeis<Operation>(exps)) {
        const Operation& op = static_cast<const Operation&>(exps);

        if (op.id() >= 0) {  // ユーザー定義関数
            // 引数をスタックにプッシュ
            for (int i = 0; i < op.argSize(); ++i)
                convertExpression(whitesp, *op.args(i));

            // 関数呼び出し
            whitesp.push(Instruments::Flow::call);
            pushInteger(whitesp, solveLabel(op.id()));
        }
        return whitesp;
    }
    // ...
}
```

### Whitespace 列

```
[arg1 の評価]
[arg2 の評価]
call [関数ラベル]   ; LF SP TB [ラベル]
```

## return 文

### 構文

```nospace
return: value;
# または
return;
```

### 生成コード

```cpp
WhiteSpace& convertReturn(WhiteSpace& whitesp, const StatementReturn& stat) {
    if (stat.retVal) {
        // 戻り値あり
        convertExpression(whitesp, *stat.retVal);
        whitesp.push(Instruments::Stack::swap);  // 戻り値と旧local_beginを入れ替え
        convertLocalDeallocate(whitesp);
    }
    else {
        // 戻り値なし
        convertLocalDeallocate(whitesp);
        whitesp.push(Instruments::Stack::push);
        pushInteger(whitesp, 0);  // デフォルト戻り値 0
    }

    whitesp.push(Instruments::Flow::retun);
    return whitesp;
}
```

### Whitespace 列（戻り値あり）

```
[戻り値の評価]
swap                ; 戻り値と旧local_beginを入れ替え
[convertLocalDeallocate]
return              ; LF TB LF
```

### Whitespace 列（戻り値なし）

```
[convertLocalDeallocate]
push 0
return              ; LF TB LF
```

## スタックフレームの詳細

### 呼び出しシーケンス

1. **呼び出し側**: 引数をスタックにプッシュ
2. **call 命令**: 戻り先アドレスをスタックにプッシュ
3. **関数開始**: `convertLocalAllocate` で旧 local_begin をスタックに退避
4. **引数処理**: スタックから引数を取り出してローカル変数に格納
5. **関数本体**: 実行
6. **関数終了**: `convertLocalDeallocate` で旧 local_begin を復元
7. **return 命令**: 戻り先へジャンプ

### スタック状態の変化

```
呼び出し前:
  スタック: [...]
  heap[LocalHeapBegin] = X
  heap[LocalHeapEnd] = Y

引数プッシュ後:
  スタック: [..., arg1, arg2]

call 後:
  スタック: [..., arg1, arg2, return_addr]

convertLocalAllocate 後:
  スタック: [..., arg1, arg2, return_addr, old_local_begin(=X)]
  heap[LocalHeapBegin] = Y
  heap[LocalHeapEnd] = Y + scope_size

引数処理後:
  スタック: [..., return_addr, old_local_begin]
  heap[Y + 0] = arg1
  heap[Y + 1] = arg2

return 前:
  スタック: [..., return_value, old_local_begin]

convertLocalDeallocate 後:
  スタック: [..., return_value]
  heap[LocalHeapBegin] = X
  heap[LocalHeapEnd] = Y

return 後:
  スタック: [..., return_value]
  (戻り先アドレスへジャンプ)
```

## main 関数

プログラムのエントリポイントは `main` 関数です。

```cpp
void attachEmbeddedFooter(WhiteSpace& code, const StatementScope& globalScope) {
    auto& mainEntry = globalScope.nameTable->getLocal("main");
    if (!typeis<NameEntryFunction>(mainEntry))
        throw GenerationException();

    code.push(Instruments::Flow::call);
    pushInteger(code, solveLabel(mainEntry.address()));
    code.push(Instruments::Flow::exit);
}
```

**Whitespace 列:**
```
call [main のラベル]
exit                ; LF LF LF
```

## 制限事項

- 再帰呼び出しは可能（スタックフレーム方式のため）
- 関数内で別の関数を定義することはできない（ネストなし）
- すべての関数は暗黙の戻り値 0 を持つ
