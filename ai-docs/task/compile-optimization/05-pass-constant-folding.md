# 定数畳み込み

## 概要

コンパイル時に評価可能な定数式を事前に計算し、`Factor(値)` に置換する。他の最適化パス（条件式最適化等）の前提となる基本的な最適化。

## 変換パターン

### 算術演算

```
Operation2(Plus, Factor(a), Factor(b))     → Factor(a + b)
Operation2(Minus, Factor(a), Factor(b))    → Factor(a - b)
Operation2(Multiply, Factor(a), Factor(b)) → Factor(a * b)
Operation2(Divide, Factor(a), Factor(b))   → Factor(a / b)  ※ b != 0
Operation2(Modulo, Factor(a), Factor(b))   → Factor(a % b)  ※ b != 0
```

### 比較演算

```
Operation2(Equal, Factor(a), Factor(b))        → Factor(if a == b { 1 } else { 0 })
Operation2(NotEqual, Factor(a), Factor(b))     → Factor(if a != b { 1 } else { 0 })
Operation2(Less, Factor(a), Factor(b))         → Factor(if a < b { 1 } else { 0 })
Operation2(LessEqual, Factor(a), Factor(b))    → Factor(if a <= b { 1 } else { 0 })
Operation2(Greater, Factor(a), Factor(b))      → Factor(if a > b { 1 } else { 0 })
Operation2(GreaterEqual, Factor(a), Factor(b)) → Factor(if a >= b { 1 } else { 0 })
```

### 単項演算

```
Operation1(Negative, Factor(a))    → Factor(-a)
Operation1(LogicalNot, Factor(a))  → Factor(if a == 0 { 1 } else { 0 })
```

### 定数条件の if

```
If(Factor(0), then_block, else_block)  → Block(else_block)   # 条件が偽
If(Factor(n), then_block, else_block)  → Block(then_block)   # n != 0, 条件が真
```

### 定数条件の while

```
While(Factor(0), body)  → Factor(0)   # ループ実行されない (while の値は 0)
```

`While(Factor(n), body)` (n != 0) は無限ループになるため、変換しない（警告を出す余地あり）。

## 再帰的な適用

定数畳み込みはボトムアップで再帰的に適用する。

```
Operation2(Plus,
    Operation2(Multiply, Factor(3), Factor(4)),  # → Factor(12)
    Factor(5)
)
→ Operation2(Plus, Factor(12), Factor(5))
→ Factor(17)
```

## 部分的な簡約

一方のオペランドだけが定数の場合の簡約（初期実装では省略可能）:

```
Operation2(Plus, expr, Factor(0))      → expr        # x + 0 = x
Operation2(Minus, expr, Factor(0))     → expr        # x - 0 = x
Operation2(Multiply, expr, Factor(0))  → Factor(0)   # x * 0 = 0
Operation2(Multiply, expr, Factor(1))  → expr        # x * 1 = x
```

## ゼロ除算の扱い

`Factor(a) / Factor(0)` や `Factor(a) % Factor(0)` は変換しない。ランタイムエラーとして残す。

## 実装手順

1. `optimizer/constant_folding.rs` を作成
2. ExecExpression を再帰的に走査する関数を実装
3. ボトムアップで定数式を評価・置換
4. 定数条件 if/while の変換
5. テスト: 定数式を含むコードで最適化結果を検証

## 他パスとの関係

- **条件式最適化の前に実行**: `if: 3 == 0 { ... }` → `if: 0 { ... }` → ブロックスコープに変換
- **未使用関数削除の前に実行**: 定数条件で分岐が除去されると、呼び出しが消える関数がある可能性
