# 短絡評価インライン化 (`short-circuit-inline`)

## 概要

`&&` / `||` 演算子の Whitespace コード生成を、サブルーチン呼び出し（`COMPARATOR_AND` / `COMPARATOR_OR`）からインライン分岐に変換する。これにより短絡評価のセマンティクスを正しく実現しつつ、実行命令数を削減する。

## 問題

### 1. 短絡評価の不在（セマンティクスの問題）

言語仕様 (`docs/spec.md`) では `&&` / `||` の短絡評価が定義されている：

> - `&&` : 短絡評価。左辺が 0 なら右辺を評価せず 0 を返す
> - `||` : 短絡評価。左辺が非0なら右辺を評価せずその値を返す

しかし現在の Whitespace コンパイラ (`src/compiler_ws/expression.rs`) では：

```rust
Operator2::LogicalAnd => {
    prog.append(generate_expression(ctx, left)?);     // 左辺を評価
    prog.append(generate_expression(ctx, right)?);    // ★ 常に右辺も評価
    prog.push(Instruction::Call(reserved_labels::COMPARATOR_AND));
}
```

**両辺を先に評価してからサブルーチンに渡している**ため、短絡評価が行われていない。右辺に副作用がある場合に仕様と異なる動作をする。

### 2. サブルーチン呼び出しのオーバーヘッド

`COMPARATOR_AND` サブルーチンは 6〜8 命令を要する：

```
Label(COMPARATOR_AND)
JumpIfZero(FALSE)      # value2 チェック
Duplicate              # ダミー
JumpIfZero(FALSE)      # value1 チェック
Discard
Push(1)
Return
Label(FALSE)
Discard
Push(0)
Return
```

Call/Return のオーバーヘッドも含め、1回の `&&` 評価に約 10 命令を消費する。

## 設計

### 変換パターン

#### `a && b` のインライン展開

```
# 最適化前（サブルーチン方式）
eval(a)
eval(b)       ← 常に評価される
Call(COMPARATOR_AND)

# 最適化後（インライン短絡評価）
eval(a)
JumpIfZero(false_label)    # a == 0 なら短絡
eval(b)
JumpIfZero(false_label)    # b == 0 なら偽
Push(1)
Jump(end_label)
Label(false_label)
Push(0)
Label(end_label)
```

#### `a || b` のインライン展開

```
# 最適化前（サブルーチン方式）
eval(a)
eval(b)       ← 常に評価される
Call(COMPARATOR_OR)

# 最適化後（インライン短絡評価）
eval(a)
JumpIfZero(check_b_label)   # a == 0 なら b をチェック
Push(1)                      # a != 0 → 短絡して真
Jump(end_label)
Label(check_b_label)
eval(b)
JumpIfZero(false_label)      # b == 0 なら偽
Push(1)
Jump(end_label)
Label(false_label)
Push(0)
Label(end_label)
```

### 実装方式の選択肢

**方式 A: コード生成時に直接インライン化**（推奨）

`src/compiler_ws/expression.rs` の `LogicalAnd` / `LogicalOr` のコード生成を修正する。最適化フラグは不要（短絡評価は仕様準拠の修正であるため、常に適用すべき）。

**方式 B: 最適化パスとして実装**

中間表現レベルで `Operation2(LogicalAnd, a, b)` を `If(Zero, a, Factor(0), If(Zero, b, Factor(0), Factor(1)))` に変換する。

→ **方式 A を推奨**。理由：短絡評価は仕様上の正しい動作であり、最適化オプションで切り替える性質のものではない。コード生成の修正で十分対応可能。

### 変更対象ファイル

| ファイル | 変更内容 |
|---|---|
| `src/compiler_ws/expression.rs` | `LogicalAnd` / `LogicalOr` のコード生成をインライン分岐に変更 |
| `src/compiler_ws/builtin.rs` | `COMPARATOR_AND` / `COMPARATOR_OR` の生成を条件付きに（他に使う箇所がなければ削除可能） |

### 命令削減量（推定）

| パターン | 最適化前 | 最適化後（最良ケース） | 最適化後（最悪ケース） |
|---|---|---|---|
| `a && b` (a == 0) | 10命令 | 3命令 | — |
| `a && b` (a != 0, b != 0) | 10命令 | — | 7命令 |
| `a \|\| b` (a != 0) | 10命令 | 3命令 | — |
| `a \|\| b` (a == 0, b != 0) | 10命令 | — | 7命令 |

短絡により右辺が評価されない場合に最も効果が大きく、特にガード条件（`if: ptr && *ptr { ... }`）で安全性にも寄与する。

## テスト

- 既存の `&&` / `||` テストケースがパスすることを確認
- 副作用を伴う短絡評価のテストケースを追加（右辺で `__puti` を呼ぶなど）
- ws_self / ws_profiler でプロファイル比較
