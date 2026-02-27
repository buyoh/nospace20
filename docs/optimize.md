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
| `dead-code` | 共通 | `__main` から到達不可能な関数をコンパイル対象から除外する |
| `peephole` | Whitespace | 生成済み命令列の冗長パターンを除去する（最終段の後処理）|

> **注**: `comparison-inline` と `discard-assign-value` はコード生成に組み込まれた常時有効な最適化であり、独立したオプションフラグを持たない。

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

`__main` 関数を起点に呼び出しグラフをたどり（BFS）、到達不可能な関数をコンパイル対象から除外する。

### 効果

- 到達不可能な関数のコード生成がスキップされ、生成コードのサイズが削減される

### 対象

- `__main` から直接・間接的に呼ばれない関数

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
5. [コード生成: comparison-inline, discard-assign-value が常時適用]
6. peephole          （最終段: 生成済み WsProgram に対して適用）
```

> `comparison-inline` と `discard-assign-value` はコード生成パスに組み込まれており、`--opt` フラグなしに常時適用される。
> `peephole` は `--opt peephole` または `--opt all` で有効化する。

---

## `comparison-inline` — 比較演算インライン化

**常時有効（オプションフラグなし）**

比較演算子（`==`, `!=`, `<`, `<=`, `>`, `>=`）のコード生成を、サブルーチン呼び出し（`COMPARATOR_ZERO` / `COMPARATOR_NEGATIVE`）からインライン分岐に変換する。

### 背景

`condition-opt` パスは if/while の条件式に直接現れる比較を最適化するが、**式として使用される比較**（例: `z = x == y;`, `f(a < b)`）は対象外だった。インライン化することで全ての比較演算でサブルーチン呼び出しが不要になる。

### 変換パターン

| 比較演算 | 変換前 | 変換後 |
|---|---|---|
| `x == y` | Push+Push+Sub+Call(COMPARATOR_ZERO) | Sub+JumpIfZero(eq)+Push(0)+Jump(end)+Label(eq)+Push(1)+Label(end) |
| `x < y` | Push+Push+Sub+Call(COMPARATOR_NEGATIVE) | Sub+JumpIfNegative(neg)+Push(0)+Jump(end)+Label(neg)+Push(1)+Label(end) |

### 命令削減量

| 比較演算 | 最適化前 | 最適化後 | 削減 |
|---|---|---|---|
| `x == y` (式として) | 11 命令 | 8 命令 | 3 命令 |
| `x < y` (式として) | 11 命令 | 8 命令 | 3 命令 |

---

## `discard-assign-value` — 代入文の値破棄最適化

**常時有効（オプションフラグなし）**

代入式 `x = expr` が文として使用される場合（結果が即座に破棄される場合）、代入後の値再取得（Retrieve）をスキップする。

### 問題

従来のコード生成では、代入は常に式としての値をスタックに残していた（Store の後に Retrieve）。文として使われる場合は直後に Discard されるため、この Retrieve が無駄だった。

```
# 従来: x = 5; のグローバル変数代入
Push(addr)  Push(5)  Store    # 代入
Push(addr)  Retrieve           # 値を再取得 (← 不要)
Discard                        # 直後に破棄

# 最適化後
Push(addr)  Push(5)  Store    # 代入のみ
```

### 命令削減量

| パターン | 変数種別 | 削減命令数 |
|---|---|---|
| `x = expr;` | グローバル | 3 命令 (Push+Retrieve+Discard) |
| `x = expr;` | ローカル | 6 命令 (Push+Push+Retrieve+Add+Retrieve+Discard) |
| `arr[i] = expr;` | グローバル | 4 命令 |
| `arr[i] = expr;` | ローカル | 7 命令 |
| `*ptr = expr;` | — | 2 命令 |

### 注意点

- 連鎖代入 `x = y = 5;` では外側の代入のみ void context となり、内側は value context のまま
- 代入式の値を使用する場合（`z = (x = 5);`）は通常の value context で処理される

---

## `peephole` — ピープホール最適化

`--opt peephole` または `--opt all` で有効化。

生成された Whitespace 命令列に対して、局所的なパターンマッチで冗長命令を除去・簡約する後処理パス。他の最適化パスの相互作用で生じる残余の冗長を回収する安全網として機能する。

### 適用パターン

| パターン | 変換前 | 変換後 |
|---|---|---|
| Push + Discard | `Push(x)` `Discard` | 削除 |
| Duplicate + Discard | `Duplicate` `Discard` | 削除 |
| Push(0) + Add | `Push(0)` `Add` | 削除（オフセット 0 のアドレス計算） |
| ジャンプ短絡 | `Jump(L1)` ... `Label(L1):Jump(L2)` | `Jump(L2)` に直接化 |
| 到達不能コード | `Jump(L)`/`Return`/`Exit` + 非ラベル命令群 | 到達不能命令を削除 |

### パイプラインの位置

```
Scope → Compiler WS → WsProgram → [Peephole] → エンコード → Whitespace 出力
```

中間表現の最適化（Phase 1/2 パス群）完了後、Whitespace エンコードの直前に適用。
