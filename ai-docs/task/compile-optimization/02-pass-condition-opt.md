# if/while 条件式最適化

## 概要

Whitespace の条件分岐命令 `JumpIfZero` / `JumpIfNegative` を直接活用し、比較サブルーチン (`COMPARATOR_ZERO`, `COMPARATOR_NEGATIVE`) の呼び出しを排除する最適化。

## 背景：現在のコード生成の非効率性

### 現在の `if: x == 0 { A } else: { B }` のコード生成

```
# 条件式 x == 0 の評価
Push(1)                     # zero_result (== 0 なら真 → 1)
Push(0)                     # nonzero_result
eval(x)                     # x の値
Push(0)                     # 右辺の 0
Sub                         # x - 0 = x
Call(COMPARATOR_ZERO)       # x==0 なら 1, x!=0 なら 0 を返す
# if 分岐
JumpIfZero(else_label)      # 結果が 0 (x!=0) なら else へ
A_block
Jump(end)
else_label:
B_block
end:
```

**問題**: `COMPARATOR_ZERO` はサブルーチン呼び出し（Call + Return + Swap + Discard）を伴い、さらにスタックに余分な値をプッシュする。合計 **約10命令** のオーバーヘッド。

### 最適化後の `If(Zero, x, A, B)` のコード生成

```
eval(x)                     # x の値
JumpIfZero(then_label)      # x==0 なら A へ直接ジャンプ
B_block                     # fall-through: x!=0
Jump(end)
then_label:
A_block
end:
```

**効果**: 比較サブルーチンなし。合計 **2命令** で分岐判定完了。

## 変換パターン

### if 文

| 元の条件式 | 変換先 | 条件式の簡約 |
|---|---|---|
| `if: expr == 0 { A } else: { B }` | `If(Zero, expr, A, B)` | == 0 の比較を排除 |
| `if: expr != 0 { A } else: { B }` | `If(Zero, expr, B, A)` | then/else を入れ替え |
| `if: expr < 0 { A } else: { B }` | `If(Negative, expr, A, B)` | < 0 の比較を排除 |
| `if: expr >= 0 { A } else: { B }` | `If(Negative, expr, B, A)` | then/else を入れ替え |
| `if: CONST { A } else: { B }` | `A` or `B` | 定数条件の除去（※） |

※ 定数条件の除去は `constant_folding` パスの責務。このパスでは非定数の条件式のみを扱う。

### while 文

| 元の条件式 | 変換先 | 説明 |
|---|---|---|
| `while: expr != 0 { body }` | `While(Zero, expr, body)` | `JumpIfZero` でループ脱出 |
| `while: expr < 0 { body }` | `While(Negative, expr, body)` | `JumpIfNegative` でループ継続 |

> **while の非対称性**: `while: expr == 0 { body }` は稀なパターンであり、初期実装では省略可能。`while: expr >= 0` も同様。

### パターンマッチの詳細

ExecExpression レベルでのパターンマッチ:

```
If(NonZero,
    cond: Operation2(Equal, inner_lhs, Factor(0)),  // expr == 0
    then_block,
    else_block
) → If(Zero, inner_lhs, then_block, else_block)
```

```
If(NonZero,
    cond: Operation2(NotEqual, inner_lhs, Factor(0)),  // expr != 0
    then_block,
    else_block
) → If(Zero, inner_lhs, else_block, then_block)  // then/else 入れ替え
```

```
If(NonZero,
    cond: Operation2(Less, inner_lhs, Factor(0)),  // expr < 0
    then_block,
    else_block
) → If(Negative, inner_lhs, then_block, else_block)
```

```
If(NonZero,
    cond: Operation2(GreaterEqual, inner_lhs, Factor(0)),  // expr >= 0
    then_block,
    else_block
) → If(Negative, inner_lhs, else_block, then_block)  // then/else 入れ替え
```

### `0 == expr` のケース

右辺がゼロでなく左辺がゼロの場合も検出する:

```
If(NonZero,
    cond: Operation2(Equal, Factor(0), inner_rhs),
    then_block,
    else_block
) → If(Zero, inner_rhs, then_block, else_block)
```

ただし `<` / `>=` は非対称なので、左右反転時は演算子を調整:

- `0 < expr` ⇔ `expr > 0` → 直接変換不可（JumpIfPositive がない）
- `0 >= expr` ⇔ `expr <= 0` → 直接変換不可

初期実装では **右辺が `Factor(0)` のケースのみ** をサポートし、左辺が `Factor(0)` のケースは `==` / `!=` のみ対応。

### `expr1 == expr2` への一般化

`expr1 == expr2` は `(expr1 - expr2) == 0` と等価。変換:

```
If(NonZero,
    cond: Operation2(Equal, lhs, rhs),
    then_block,
    else_block
)
→ If(Zero, Operation2(Minus, lhs, rhs), then_block, else_block)
```

しかし、これは `lhs - rhs` の計算を生成するため、元の `Sub + Call(COMPARATOR_ZERO)` と比較して `Sub` は同じで `Call` が消える分だけ得する。**初期実装で対応推奨**。

同様に:

- `expr1 != expr2` → `If(Zero, expr1 - expr2, else, then)`
- `expr1 < expr2` → `If(Negative, expr1 - expr2, then, else)`
- `expr1 >= expr2` → `If(Negative, expr1 - expr2, else, then)`
- `expr1 > expr2` → `If(Negative, expr2 - expr1, then, else)` （オペランド反転）
- `expr1 <= expr2` → `If(Negative, expr2 - expr1, else, then)` （オペランド反転 + then/else 反転）

## Compiler WS でのコード生成

既存の `generate_if_expression` / `generate_while_expression` を `ConditionMode` に対応させる。

### If の ConditionMode 対応

```rust
// generate_if_expression(ctx, mode, cond, then_block, else_block)
fn generate_if_expression(
    ctx: &mut CodeGenContext,
    mode: &ConditionMode,
    cond: &LocatedExecExpression,
    then_block: &Block,
    else_block: &Block,
) -> Result<WsProgram, CompileError> {
    match mode {
        ConditionMode::NonZero => {
            // 既存実装: COMPARATOR 経由
            generate_if_nonzero(ctx, cond, then_block, else_block)
        }
        ConditionMode::Zero => {
            let then_label = ctx.new_label();
            let end_label = ctx.new_label();
            prog.append(generate_expression(ctx, cond)?);
            prog.push(Instruction::JumpIfZero(then_label));
            prog.append(generate_block(ctx, else_block)?);
            prog.push(Instruction::Jump(end_label));
            prog.push(Instruction::Label(then_label));
            prog.append(generate_block(ctx, then_block)?);
            prog.push(Instruction::Label(end_label));
            Ok(prog)
        }
        ConditionMode::Negative => {
            let then_label = ctx.new_label();
            let end_label = ctx.new_label();
            prog.append(generate_expression(ctx, cond)?);
            prog.push(Instruction::JumpIfNegative(then_label));
            prog.append(generate_block(ctx, else_block)?);
            prog.push(Instruction::Jump(end_label));
            prog.push(Instruction::Label(then_label));
            prog.append(generate_block(ctx, then_block)?);
            prog.push(Instruction::Label(end_label));
            Ok(prog)
        }
    }
}
```

### While の ConditionMode 対応

```rust
// generate_while_expression(ctx, mode, cond, body)
match mode {
    ConditionMode::NonZero => {
        // 既存実装: COMPARATOR 経由
        generate_while_nonzero(ctx, cond, body)
    }
    ConditionMode::Zero => {
        // cond == 0 で継続 → JumpIfZero でループ脱出
        prog.push(Instruction::Label(loop_start));
        prog.append(generate_expression(ctx, cond)?);
        prog.push(Instruction::JumpIfZero(loop_end));  // cond == 0 → exit
        prog.append(generate_block(ctx, body)?);
        prog.push(Instruction::Discard);
        prog.push(Instruction::Jump(loop_start));
        prog.push(Instruction::Label(loop_end));
    }
    ConditionMode::Negative => {
        // cond < 0 で継続 → JumpIfNegative でループ本体へ
        prog.push(Instruction::Label(loop_start));
        prog.append(generate_expression(ctx, cond)?);
        prog.push(Instruction::JumpIfNegative(loop_body));
        prog.push(Instruction::Jump(loop_end));
        prog.push(Instruction::Label(loop_body));
        prog.append(generate_block(ctx, body)?);
        prog.push(Instruction::Discard);
        prog.push(Instruction::Jump(loop_start));
        prog.push(Instruction::Label(loop_end));
    }
}
```

## 命令数の削減効果（見積もり）

| パターン | 最適化前 (命令数) | 最適化後 (命令数) | 削減 |
|---|---|---|---|
| `if: x == 0` | Push×2 + eval(x) + Push + Sub + Call + JumpIfZero (≈10+) | eval(x) + JumpIfZero (≈2) | ~8 |
| `if: x < 0` | Push×2 + eval(x) + Push + Sub + Call + JumpIfZero (≈10+) | eval(x) + JumpIfNegative (≈2) | ~8 |
| `if: a == b` | Push×2 + eval(a) + eval(b) + Sub + Call + JumpIfZero (≈9+) | eval(a) + eval(b) + Sub + JumpIfZero (≈4) | ~5 |
| `while: x != 0` | 同上（ループ毎） | eval(x) + JumpIfZero (≈2) | ~8/iteration |

## 実装手順

### 前提: ConditionMode 導入リファクタリング (01-pass-framework.md 参照)

1. `ConditionMode` enum を `types.rs` に追加
2. `ExecExpression::If` / `While` に `ConditionMode` フィールドを追加
3. `semantic_analyzer/mod.rs` で `ConditionMode::NonZero` を指定するよう修正
4. `interpreter/exec.rs` の match パターンを更新（ConditionMode 対応）
5. `compiler_ws/expression.rs` の match パターンを更新（ConditionMode 対応）
6. テスト: 既存テスト全パスを確認（動作変更なし）

### 条件式最適化パスの実装

1. `optimizer/condition_opt.rs` を作成
2. `If(NonZero, ...)` → `If(Zero/Negative, ...)` のパターンマッチ・変換ロジックを実装
3. `While(NonZero, ...)` → `While(Zero, ...)` 等の変換ロジックを実装
4. テスト: 既存テストケースが最適化有無で同じ結果になることを確認
5. プロファイラで効果測定

## 注意事項

- `LogicalAnd` / `LogicalOr` と組み合わさった条件式は初期実装では対象外。`if: x == 0 && y > 0` のような複合条件は変換しない。
- 最適化はオプショナルであり、最適化なしでも正しいコードが生成されることを保証する。
