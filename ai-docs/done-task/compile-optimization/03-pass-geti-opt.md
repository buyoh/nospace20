# `__geti` / `__getc` 入力最適化

> **✅ 実装完了** (2026-02-27)
> - `src/optimizer/geti_opt.rs` 作成
> - `OptimizationOptions::geti_opt` フィールド追加
> - `optimizer::tests` に 7 件のテスト追加（合計 38 件）
> - 全テスト 990 passed, 0 failed

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

### 最適化後の `InternalBuiltinFunction(Getiv(p))` のコード生成

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
→ ExecStatement::Expression(InternalBuiltinFunction(Getiv(var_ref)))
```

```
ExecStatement::Expression(
    Operation2(Assign,
        Variable(var_ref),                    # 左辺: 変数
        BuiltinFunction(Getc, [])             # 右辺: __getc()
    )
)
→ ExecStatement::Expression(InternalBuiltinFunction(Getcv(var_ref)))
```

### 適用条件

- 左辺が単純な変数参照 (`Variable(IdentifierRef)`) であること
  - 配列アクセス (`ArrayAccess`) やデリファレンス (`*ptr`) は対象外（初期実装）
- 右辺が引数なしの `__geti()` または `__getc()` であること
- 代入式の戻り値が使用されていること（ExecStatement::Expression の場合、戻り値は通常破棄されるが、式として使用される場合もある）

### 戻り値の扱い

`p = __geti()` が式として使われる場合（例: `x = p = __geti()`）、戻り値がスタックに残る必要がある。`InternalBuiltinFunction(Getiv(...))` は最適化後もスタックに入力値を残す。

ただし、以下のような複雑なケースは変換しない:

```
# 変換しない: ネストした代入
x = (p = __geti()) + 1;
```

初期実装では **文の直接の式** (`ExecStatement::Expression`) でのみパターンマッチする。

## Compiler WS でのコード生成

### InternalBuiltinFunction(Getiv(...))（グローバル変数）

```rust
ExecExpression::InternalBuiltinFunction(kind) => match kind {
    InternalBuiltinFunctionKind::Getiv(var_ref) | InternalBuiltinFunctionKind::Getcv(var_ref) => {
        let is_number = matches!(kind, InternalBuiltinFunctionKind::Getiv(_));
        let var_info = ctx.get_var_info(var_ref);
        match var_info.scope {
            VarScope::Global => {
                let addr = heap_layout::GLOBAL_PTR + var_info.offset;
                prog.push(Instruction::Push(WsNumber(addr)));
                prog.push(Instruction::Duplicate);
                if is_number { prog.push(Instruction::InputNumber); }
                else { prog.push(Instruction::InputChar); }
                prog.push(Instruction::Retrieve);
            }
            VarScope::Local => {
                prog.push(Instruction::Push(WsNumber(var_info.offset)));
                prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
                prog.push(Instruction::Retrieve);
                prog.push(Instruction::Add);
                prog.push(Instruction::Duplicate);
                if is_number { prog.push(Instruction::InputNumber); }
                else { prog.push(Instruction::InputChar); }
                prog.push(Instruction::Retrieve);
            }
        }
    }
}
```

## Interpreter での処理

```rust
ExecExpression::InternalBuiltinFunction(kind) => match kind {
    InternalBuiltinFunctionKind::Getiv(var_ref) => {
        let value = read_integer_from_stdin();
        set_variable(var_ref, value);
        ExpressionFlow::Value(value)
    }
    InternalBuiltinFunctionKind::Getcv(var_ref) => {
        let value = read_char_from_stdin();
        set_variable(var_ref, value);
        ExpressionFlow::Value(value)
    }
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

### 前提: InternalBuiltinFunction 導入 (01-pass-framework.md 参照)

1. `InternalBuiltinFunctionKind` enum を `types.rs` に追加
2. `ExecExpression::InternalBuiltinFunction` バリアントを追加
3. `types.rs` の `infer_type` に新バリアントを追加
4. Interpreter にハンドラを追加
5. Compiler WS にコード生成を追加

### 最適化パスの実装

1. `optimizer/geti_opt.rs` にパターンマッチ・変換ロジックを実装
2. テスト: `__geti`/`__getc` を使用するテストケースで最適化有無で同じ結果を確認

## 将来の拡張

`InternalBuiltinFunctionKind` に新しいバリアントを追加することで、`ExecExpression` を変更せずに拡張可能:

- 配列アクセスへの直接入力: `arr[i] = __geti()` → `GetivArray(IdentifierRef, Box<LocatedExecExpression>)`
- デリファレンスへの直接入力: `*ptr = __geti()` → `GetivDeref(Box<LocatedExecExpression>)`
