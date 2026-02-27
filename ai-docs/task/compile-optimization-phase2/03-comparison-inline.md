# 比較演算インライン化 (`comparison-inline`)

## 概要

比較演算子（`==`, `!=`, `<`, `<=`, `>`, `>=`）のコード生成を、サブルーチン呼び出し（`COMPARATOR_ZERO` / `COMPARATOR_NEGATIVE`）からインライン分岐に変換する。

## 背景

`condition-opt` パスは **if/while の条件式** に直接現れる比較演算を最適化するが、**式として使用される比較**（例: `z = x == y;`, `f(a < b)`）は対象外のままである。

現在の比較演算コード生成パターン（例: `x == y`）：

```
Push(1)            # true の場合の値
Push(0)            # false の場合の値
eval(x)
eval(y)
Sub                # x - y
Call(COMPARATOR_ZERO)   # ← サブルーチン呼び出し
```

`COMPARATOR_ZERO` サブルーチン内部（6命令）：

```
Label(COMPARATOR_ZERO)
JumpIfZero(ZERO_2)    # value == 0 → ゼロ結果
Swap                  # value != 0 → 非ゼロ結果
Label(ZERO_2)
Discard               # 不要な値を破棄
Return
```

合計：前処理 5命令 + Call 1命令 + サブルーチン 4命令 + Return 1命令 = **約 11 命令**。

## 設計

### インライン変換パターン

#### `x == y` → インライン JumpIfZero

```
eval(x)
eval(y)
Sub
JumpIfZero(equal_label)
Push(0)                    # x != y → false
Jump(end_label)
Label(equal_label)
Push(1)                    # x == y → true
Label(end_label)
```

合計 **8 命令**（Call/Return のオーバーヘッド排除で約 3 命令削減）。

#### `x != y` → インライン JumpIfZero

```
eval(x)
eval(y)
Sub
JumpIfZero(equal_label)
Push(1)                    # x != y → true
Jump(end_label)
Label(equal_label)
Push(0)                    # x == y → false
Label(end_label)
```

#### `x < y` → インライン JumpIfNegative

```
eval(x)
eval(y)
Sub
JumpIfNegative(neg_label)
Push(0)                    # x >= y → false
Jump(end_label)
Label(neg_label)
Push(1)                    # x < y → true
Label(end_label)
```

他の比較演算子も同様のパターンで展開。

### 実装方式の選択肢

**方式 A: コード生成レベルで直接インライン化**

`src/compiler_ws/expression.rs` の `generate_binary_op` で比較演算コード生成を直接変更する。

**方式 B: 最適化パスとして中間表現を変換**

→ 中間表現に比較結果のインライン化を表現する適切なバリアントが必要。`condition-opt` とは異なるアプローチが必要。

→ **方式 A を推奨**。コード生成の変更で完結し、最適化オプションで切り替えは不要（サブルーチン方式より常に効率的）。

### 変更対象ファイル

| ファイル | 変更内容 |
|---|---|
| `src/compiler_ws/expression.rs` | 比較演算（Equal, NotEqual, Less, LessEqual, Greater, GreaterEqual）のコード生成をインライン分岐に変更 |
| `src/compiler_ws/builtin.rs` | `COMPARATOR_ZERO` / `COMPARATOR_NEGATIVE` サブルーチンの生成を条件付きに（condition-opt で使わなくなる場合のみ削除可能） |

### 命令削減量（推定）

| パターン | 最適化前 | 最適化後 | 削減 |
|---|---|---|---|
| `x == y` (式として) | 11命令 | 8命令 | 3命令 |
| `x < y` (式として) | 11命令 | 8命令 | 3命令 |

1 回の削減は小さいが、ループ内の比較での累積効果は大きい。`condition-opt` との相乗効果あり。

### サブルーチン削除の検討

`condition-opt` と `comparison-inline` の両方が有効な場合、`COMPARATOR_ZERO` / `COMPARATOR_NEGATIVE` を呼び出す箇所がなくなる可能性がある。`LogicalNot` (`!x`) のみサブルーチンを使用するため、これもインライン化すれば完全に削除可能。

## テスト

- 既存テスト全通過の確認
- 比較演算の結果が式として正しく使えることの確認（代入、関数引数、ネスト）
- プロファイル比較

## ドキュメント更新

- `docs/optimize.md` に `comparison-inline` パスの説明セクションを追加
- パス一覧テーブルへの追記
- パスの実行順序への追記
