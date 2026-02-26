# 最適化オプション

nospace20 コンパイラ・インタプリタが持つ最適化パスの説明。

## 使い方

`--opt <パス名>` を複数回指定することで、任意の組み合わせで最適化を有効化できる。

```bash
# すべての実用的な最適化を有効化
nospace20 --opt all program.ns

# 定数畳み込みと未使用関数削除のみ有効化
nospace20 --opt constant-folding --opt dead-code program.ns

# コンパイルモードでも同様に指定できる
nospace20 --mode compile --opt all -o out.ws program.ns
```

## パス一覧

| パス名 | 対象バックエンド | 効果 |
|---|---|---|
| `all` | — | 以下のすべてのパスを有効化（ショートカット） |
| `constant-folding` | 共通 | コンパイル時に評価可能な定数式を事前計算して置換する |
| `condition-opt` | Whitespace | if/while の条件式を JumpIfZero/JumpIfNegative に直接変換する |
| `geti-opt` | Whitespace | `p = __geti()` / `p = __getc()` の一時領域経由を排除する |
| `dead-code` | 共通 | `main` から到達不可能な関数をコンパイル対象から除外する |

---

## `constant-folding` — 定数畳み込み

コンパイル時に評価可能な定数式をボトムアップで計算し、`Factor(値)` に置換する。

### 対象パターン

**算術演算**:

```
3 + 4    → 7
3 * 4    → 12
10 / 2   → 5
10 % 3   → 1
-5       → -5
```

**比較演算** (結果は 0 または 1):

```
3 == 3   → 1
3 != 3   → 0
3 < 5    → 1
```

**論理否定**:

```
!0       → 1
!1       → 0
```

**定数条件の if**:

```
if: 0 { A } else: { B }    → B のみ残る
if: 1 { A } else: { B }    → A のみ残る
```

**定数条件の while** (実行されないループの除去):

```
while: 0 { body }   → 除去
```

### 効果

- 他のパスの前提となる基本的な変換
- 定数条件の分岐を除去することで、後段の `dead-code` パスが未使用関数をさらに検出しやすくなる

---

## `condition-opt` — 条件式最適化

Whitespace バックエンドに固有の最適化。if/while の条件式を Whitespace の `JumpIfZero` / `JumpIfNegative` 命令に直接マッピングし、比較サブルーチン呼び出しを排除する。

### 背景

最適化前の `if: x == 0 { A } else: { B }` は比較サブルーチン (`COMPARATOR_ZERO`) を経由して約 10 命令を要する。最適化後は条件式の評価と分岐を合わせて 2 命令に短縮される。

### 変換パターン

| 条件式 | 変換先 | 説明 |
|---|---|---|
| `expr == 0` | `JumpIfZero` | そのまま |
| `expr != 0` | `JumpIfZero` | then/else を入れ替え |
| `expr < 0` | `JumpIfNegative` | そのまま |
| `expr >= 0` | `JumpIfNegative` | then/else を入れ替え |
| `lhs == rhs` | `JumpIfZero(lhs - rhs)` | 差を評価 |
| `lhs != rhs` | `JumpIfZero(lhs - rhs)` | 差を評価 + then/else 入れ替え |
| `lhs < rhs` | `JumpIfNegative(lhs - rhs)` | 差を評価 |
| `lhs >= rhs` | `JumpIfNegative(lhs - rhs)` | 差を評価 + then/else 入れ替え |
| `lhs > rhs` | `JumpIfNegative(rhs - lhs)` | オペランド反転 |
| `lhs <= rhs` | `JumpIfNegative(rhs - lhs)` | オペランド反転 + then/else 入れ替え |

while 文も同様のパターンに対応する。

### 命令削減例

```
# 最適化前: x == 0 の分岐（約 10 命令）
Push(1); Push(0); eval(x); Push(0); Sub; Call(COMPARATOR_ZERO); JumpIfZero(else)

# 最適化後（2 命令）
eval(x); JumpIfZero(then)
```

---

## `geti-opt` — `__geti`/`__getc` 入力最適化

Whitespace バックエンドに固有の最適化。`p = __geti()` / `p = __getc()` パターンを検出し、一時領域 (`TEMP_PTR`) を経由せずに変数アドレスへ直接入力する命令列に変換する。

### 背景

最適化前は一時領域に入力してから変数にコピーする（9〜13 命令）が、最適化後は変数アドレスに直接入力する（4〜7 命令）。

### 命令削減量

| パターン | 変数種別 | 最適化前 | 最適化後 | 削減 |
|---|---|---|---|---|
| `p = __geti()` | グローバル | 9 命令 | 4 命令 | 5 |
| `p = __geti()` | ローカル | 13 命令 | 7 命令 | 6 |
| `p = __getc()` | グローバル | 9 命令 | 4 命令 | 5 |
| `p = __getc()` | ローカル | 13 命令 | 7 命令 | 6 |

### 適用条件

- 左辺が単純な変数参照であること（配列アクセスや `*ptr` は対象外）
- 右辺が引数なしの `__geti()` または `__getc()` であること
- 文の直接の式である（ネストした代入は対象外）こと

---

## `dead-code` — 未使用関数削除

`main` 関数を起点に呼び出しグラフをたどり（BFS）、到達不可能な関数をコンパイル対象から除外する。

### 効果

- 到達不可能な関数のコード生成がスキップされ、生成コードのサイズが削減される

### 対象

- `main` から直接・間接的に呼ばれない関数

グローバル変数の初期化式から呼ばれる関数も「到達可能」とみなす。

> **未使用変数の削除は現在未対応。**

---

## パスの実行順序

複数のパスを有効化した場合、以下の固定順序で実行される。

```
1. constant-folding  （条件式最適化の前提となるため最初）
2. condition-opt
3. geti-opt
4. dead-code
```
