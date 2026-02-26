# `__geti` / `__getc` 入力最適化

## 概要

`p = __geti()` / `p = __getc()` パターンを検出し、一時領域 (`TEMP_PTR`) を経由せずに変数アドレスへ直接入力する命令列に変換する。

## 背景：現在のコード生成の非効率性

### 現在の `p = __geti()` のコード生成（グローバル変数の場合）

```
# __geti() の評価
Push(TEMP_PTR)          # 一時領域アドレス
Duplicate               # アドレスを複製
InputNumber             # heap[TEMP_PTR] = 入力値
Retrieve                # スタックに入力値を取得

# p への代入 (Assign)
Push(p_addr)            # p のアドレス
Swap                    # addr, value の順に
Store                   # heap[p_addr] = value
Push(p_addr)            # 代入式の戻り値取得
Retrieve                # value をスタックに

合計: 9 命令
```

### 最適化後の `InternalGetiv(p)` のコード生成

```
Push(p_addr)            # p のアドレス
Duplicate               # アドレスを複製
InputNumber             # heap[p_addr] = 入力値（直接）
Retrieve                # スタックに入力値を取得

合計: 4 命令
```

**効果**: 一時領域へのラウンドトリップと代入コードが不要になり、**5命令削減**。

## 変換パターン

### パターンマッチ

ExecStatement レベルで以下のパターンを検出:

```
ExecStatement::Expression(
    Operation2(Assign,
        Variable(var_ref),                    # 左辺: 変数
        BuiltinFunction(Geti, [])             # 右辺: __geti()
    )
)
→ ExecStatement::Expression(InternalGetiv(var_ref))
```

```
ExecStatement::Expression(
    Operation2(Assign,
        Variable(var_ref),                    # 左辺: 変数
        BuiltinFunction(Getc, [])             # 右辺: __getc()
    )
)
→ ExecStatement::Expression(InternalGetcv(var_ref))
```

### 適用条件

- 左辺が単純な変数参照 (`Variable(IdentifierRef)`) であること
  - 配列アクセス (`ArrayAccess`) やデリファレンス (`*ptr`) は対象外（初期実装）
- 右辺が引数なしの `__geti()` または `__getc()` であること
- 代入式の戻り値が使用されていること（ExecStatement::Expression の場合、戻り値は通常破棄されるが、式として使用される場合もある）

### 戻り値の扱い

`p = __geti()` が式として使われる場合（例: `x = p = __geti()`）、戻り値がスタックに残る必要がある。`InternalGetiv` は最適化後もスタックに入力値を残す。

ただし、以下のような複雑なケースは変換しない:

```
# 変換しない: ネストした代入
x = (p = __geti()) + 1;
```

初期実装では **文の直接の式** (`ExecStatement::Expression`) でのみパターンマッチする。

## Compiler WS でのコード生成

### InternalGetiv（グローバル変数）

```rust
ExecExpression::InternalGetiv(var_ref) => {
    let var_info = ctx.get_var_info(var_ref);
    match var_info.scope {
        VarScope::Global => {
            let addr = heap_layout::GLOBAL_PTR + var_info.offset;
            prog.push(Instruction::Push(WsNumber(addr)));
            prog.push(Instruction::Duplicate);
            prog.push(Instruction::InputNumber);
            prog.push(Instruction::Retrieve);
        }
        VarScope::Local => {
            // local_addr = heap[LOCAL_HEAP_BEGIN] + offset
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
            prog.push(Instruction::Duplicate);
            prog.push(Instruction::InputNumber);
            prog.push(Instruction::Retrieve);
        }
    }
}
```

### InternalGetcv

`InputNumber` を `InputChar` に置き換えるだけで同じ構造。

## Interpreter での処理

```rust
ExecExpression::InternalGetiv(var_ref) => {
    // __geti() と同じ処理 + 変数への代入
    let value = read_integer_from_stdin();
    set_variable(var_ref, value);
    ExpressionFlow::Value(value)
}
ExecExpression::InternalGetcv(var_ref) => {
    let value = read_char_from_stdin();
    set_variable(var_ref, value);
    ExpressionFlow::Value(value)
}
```

## 命令数削減効果

| パターン | 変数種別 | 最適化前 | 最適化後 | 削減 |
|---|---|---|---|---|
| `p = __geti()` | グローバル | 9 命令 | 4 命令 | 5 |
| `p = __geti()` | ローカル | 13 命令 | 7 命令 | 6 |
| `p = __getc()` | グローバル | 9 命令 | 4 命令 | 5 |
| `p = __getc()` | ローカル | 13 命令 | 7 命令 | 6 |

## 実装手順

1. `ExecExpression` に `InternalGetiv(IdentifierRef)`, `InternalGetcv(IdentifierRef)` を追加
2. `types.rs` の `infer_type` に新バリアントを追加
3. Interpreter に新バリアントのハンドラを追加
4. Compiler WS (`expression.rs`) に新バリアントのコード生成を追加
5. `optimizer/geti_opt.rs` にパターンマッチ・変換ロジックを実装
6. テスト: `__geti`/`__getc` を使用するテストケースで最適化有無で同じ結果を確認

## 将来の拡張

- 配列アクセスへの直接入力: `arr[i] = __geti()` → `InternalGetivArray(var_ref, index_expr)`
- デリファレンスへの直接入力: `*ptr = __geti()` → `InternalGetivDeref(addr_expr)`
