# 未実装の変数関連機能

このドキュメントは nospace プログラミング言語における未実装の変数関連機能をまとめたものです。

最終更新日: 2026-02-28

## 目次

1. [alias（エイリアス）](#1-aliasエイリアス)
2. [constexpr（コンパイル時定数エイリアス）](#2-constexprコンパイル時定数エイリアス)
3. [final 変数](#3-final-変数)
4. [実装計画](#4-実装計画)
5. [設計上の未決定事項](#5-設計上の未決定事項)

> **注記**: Step 1（pure_eval）、Step 2（constexpr）、Step 3（識別子alias）、Step 4（ブロックalias）、Step 5（final変数）、Step 7（spec反映）は完了。残りは Step 6（constexprブロック形式）のみ。

---

## 1. alias（エイリアス）

**状態**: ✅ 実装済み（識別子エイリアスのみ。ブロックエイリアスは Step 4）

**実装内容**:
- `token_parser/mod.rs`: `Keyword::Alias` を追加
- `tree_parser/statement/mod.rs`: `Statement::AliasIdentifier(String, String)` を追加、パーサー実装
- `semantic_analyzer/scope.rs`: `ScopeInfo::alias_map` フィールド追加、`ScopeResolver::resolve_alias_chain` メソッド追加（巡回参照検知付き）
- `semantic_analyzer/mod.rs`: Pass 0 に alias 収集追加（`collect_alias_map`）、変数/関数参照時に alias チェーン解決
- テストケース追加: `alias_var_001`, `alias_func_001`, `alias_chain_001`, `alias_forward_ref_001`（成功）、`alias_circular_001`（コンパイルエラー）

**制限事項**:
- `for:` の初期化ブロック内の alias は同 for の条件・更新・本体ブロックからは不可視（空の alias テーブルを渡している）
- `&alias_var` のアドレス参照は未対応（`Expression::Operation1(Ref, Variable)` でのエイリアス解決が必要）

**説明**: コンパイル時の名前置換機構。識別子名のエイリアス、またはブロックスコープの置換を定義する。
alias はランタイム実体を持たず、コンパイル時に完全に解決される。

### 1.1 識別子エイリアス

既存の識別子（関数名・変数名）に別名を付ける。

```nospace
func: func1() { return: 42; }
alias: afunc(func1);

func: __main() {
  __puti(afunc());  # func1() と同じ。42 を出力 #
}
```

```nospace
func: __main() {
  let: x(10);
  alias: y(x);
  y = 20;            # x = 20 と同じ #
  __assert(x == 20);
}
```

**動作**:
- `alias: name(target)` で、`name` を `target` の別名として登録する
- `name` が使用された箇所で `target` に名前解決される
- target が関数なら関数として、変数なら変数として扱われる
- エイリアスのエイリアスも可能（チェーン解決）
  - `alias: a(b); alias: b(c);` → `a` は最終的に `c` に解決

### 1.2 ブロックエイリアス

ブロック（文の列）を名前に紐付け、呼び出し時に展開する。
引数なしの関数呼び出し構文で使用する。

```nospace
func: func1() { return: 0; }

alias: greet {
  __puti(func1());
};

func: __main() {
  greet();  # ブロックが展開される #
}
```

**動作**:
- `alias: name { 文... }` で、ブロックを `name` に紐付ける
- `name()` の呼び出しで、ブロックの AST がインライン展開される
- ブロック内の名前解決は**呼び出し元のスコープ**で行われる（マクロ的置換）
- 引数は取れない（引数が必要なら `func:` を使用する）
- 戻り値はブロックの最後の式の値（ブロック式と同じ）

**ブロック展開のスコープモデル**:
ブロックエイリアスはマクロ的な展開（テキスト置換の AST 版）として動作する。
tree_parser で AST として保存し、呼び出し箇所で AST をクローンして挿入し、
semantic_analyzer が呼び出し元のスコープのコンテキストで名前解決を行う。

```nospace
func: __main() {
  let: x(5);
  alias: inc_x { x = x + 1; };
  inc_x();
  __assert(x == 6);  # 呼び出し元の x が変更される #
}
```

### 1.3 巡回参照の検知

**問題**: エイリアスが巡回参照を形成した場合、無限ループに陥る。

#### 識別子エイリアスの巡回検知

```nospace
alias: a(b);
alias: b(a);  # 巡回参照: a → b → a #
```

**検知方法**: エイリアス解決時に訪問済みセットを使用する。

```
resolve_alias(name, visited):
  if name ∈ alias_map:
    if name ∈ visited:
      → コンパイルエラー: "circular alias reference detected: {chain}"
    visited ← visited ∪ {name}
    return resolve_alias(alias_map[name].target, visited)
  return name
```

- 計算量: O(チェーン長) — エイリアスチェーンの長さに比例
- **検知は容易**: 訪問済みセットの確認だけで十分
- エラー時にチェーン情報を表示可能（例: `a → b → a`）

#### ブロックエイリアスの巡回検知

```nospace
alias: a { b(); };
alias: b { a(); };  # 巡回参照: a の展開中に b → a #
```

**検知方法**: 展開スタック（expanding stack）を使用する。

```
expand_block_alias(name, expanding_stack):
  if name ∈ expanding_stack:
    → コンパイルエラー: "recursive block alias expansion: {stack}"
  expanding_stack.push(name)
  # ブロック内のすべての呼び出しを解析
  for each call in block:
    if call.target ∈ block_alias_map:
      expand_block_alias(call.target, expanding_stack)
  expanding_stack.pop()
```

- semantic_analyzer の式変換時に展開スタックを管理
- ブロック内の関数呼び出しがブロックエイリアスを参照する場合、再帰的にチェック
- **検知は中程度の難易度**: 展開スタックの管理が必要だが、アルゴリズムは単純

#### 識別子・ブロック混合の巡回

```nospace
alias: a(b);
alias: b { a(); };  # 識別子 a → 変数/関数 b? → ブロック b? #
```

- 識別子エイリアスとブロックエイリアスは名前空間が異なる可能性があるが、
  同一名前空間で管理する場合、混合巡回も発生しうる
- 統一的な解決チェーンで検知可能

### 1.4 ホイスティング

- alias はスコープ内でホイスティングされる（let/func と同様）
- alias 定義より前で使用可能
- ただし、alias のターゲットが未定義の場合はコンパイルエラー

---

## 2. constexpr（コンパイル時定数エイリアス）

**状態**: ✅ 実装済み

**実装内容**:
- `token_parser/mod.rs`: `Keyword::Constexpr` を追加
- `tree_parser/statement/mod.rs`: `Statement::ConstexprDeclaration(String, Box<LocatedExpression>)` を追加、パーサー実装
- `semantic_analyzer/scope.rs`: `ScopeInfo::constexpr_table` フィールド追加、`ScopeResolver::resolve_constexpr` メソッド追加
- `semantic_analyzer/mod.rs`: Pass0（constexpr 収集・評価）追加、式変換時に constexpr → `Factor(value)` に置換
- テストケース追加: `constexpr_basic_001`, `constexpr_expr_001`, `constexpr_chain_001`, `constexpr_forward_ref_001`（成功）、`constexpr_circular_001`, `constexpr_non_const_var_001`, `constexpr_non_const_func_001`（コンパイルエラー）

**制限事項**:
- `for:` の初期化ブロック内の constexpr は、同 for の条件・更新・本体ブロックからは不可視（空の constexpr テーブルを渡している）

**旧設計との違い**:
- 旧設計: `const` は再代入不可の変数（スタックスロットを確保）
- 新設計: `constexpr` はコンパイル時に解決される定数エイリアス（変数ではない）
- `constexpr` は `alias` の制約付きバージョンとして位置づけられる

### 2.1 構文

```nospace
constexpr: PI(3);                  # リテラル定数 #
constexpr: SIZE(2 + 3);            # コンパイル時式（= 5 に解決）#
constexpr: DOUBLE_PI(PI * 2);      # 他の constexpr を参照可能 #
# constexpr: X(variable);          # コンパイルエラー: 非定数式 #
# constexpr: Y(func1());           # コンパイルエラー: 関数呼び出しは非定数 #
```

### 2.2 セマンティクス

- `constexpr: name(expr)` で `name` をコンパイル時定数として定義
- `expr` はコンパイル時に評価可能でなければならない（定数式）
- 評価結果は単一の整数値（`Factor(value)` に置換）
- スタックスロットを確保しない（変数ではない）
- `&name` は不可（アドレスが存在しない）
- 代入 `name = ...` は不可（定数であり変数ではない）

### 2.3 定数式（constexpr）の評価

コンパイル時に評価可能な式：

| 式の種類 | 評価可能 | 備考 |
|----------|---------|------|
| 数値リテラル | ✅ | `42`, `0xFF` |
| 文字リテラル | ✅ | `'A'`, `'\n'` |
| 他の constexpr 参照 | ✅ | 解決済みの値を使用 |
| 算術演算 (+, -, *, /, %) | ✅ | オペランドが定数式の場合 |
| 比較演算 (==, !=, <, <=, >, >=) | ✅ | オペランドが定数式の場合 |
| 論理演算 (&&, \|\|, !) | ✅ | オペランドが定数式の場合 |
| 変数参照 | ❌ | ランタイム値 |
| 関数呼び出し | ❌ | 副作用の可能性 |
| if / ブロック | ❌ | 制御フローは非定数 |

**評価器の実装**:

```
evaluate_constexpr(expr, constexpr_table) -> Result<i64, Error>:
  match expr:
    Factor(n) → Ok(n)
    Variable(name):
      if name ∈ constexpr_table:
        Ok(constexpr_table[name])
      else:
        Err("not a compile-time constant")
    Operation1(Neg, e) → Ok(-evaluate_constexpr(e, const_table)?)
    Operation1(Not, e) → Ok(if evaluate_constexpr(e, const_table)? == 0 then 1 else 0)
    Operation2(Plus, l, r) → Ok(evaluate_constexpr(l)? + evaluate_constexpr(r)?)
    # ... 他の演算も同様
    _ → Err("expression is not compile-time evaluable")
```

- 既存の最適化パスの定数畳み込み（constant_folding）と類似のロジック
- 0 除算はコンパイルエラーとして報告

### 2.4 constexpr ブロック形式（将来拡張）

```nospace
# 将来的に検討 #
constexpr: VALUE {
  let: tmp(3);
  tmp * tmp;   # = 9 #
};
```

ブロック内の全ての処理がコンパイル時に評価可能な場合のみ許可。
初回実装では式形式のみをサポートし、ブロック形式は将来拡張とする。

### 2.5 constexpr のホイスティングと定義順序

- constexpr はスコープ内でホイスティングされる
- ただし、constexpr の初期化式内で別の constexpr を参照する場合、
  参照先の constexpr が先に定義されている（or 同一スコープ内で定義されている）必要がある
- 巡回参照はコンパイルエラー

```nospace
constexpr: A(B + 1);  # OK: B は同一スコープ内で定義 #
constexpr: B(10);

# constexpr: X(Y + 1);  巡回参照エラー #
# constexpr: Y(X + 1);  #
```

---

## 3. final 変数

**状態**: ✅ 実装済み（Step 5 完了）

**実装内容**:
- `token_parser/mod.rs`: `Keyword::Final` を追加
- `tree_parser/statement/mod.rs`: `Statement::VariableDeclaration` に `is_final: bool` フラグ追加、`final:` のパース処理追加
- `semantic_analyzer/types.rs`: `Variable` 構造体に `is_final: bool` フィールド追加
- `semantic_analyzer/scope.rs`: `is_final_variable()` メソッド追加
- `semantic_analyzer/mod.rs`: `Operator2::Assign` 時に final 変数への代入チェック（複合代入 +=, -= 等も含む）追加
- テストケース追加: `var_final_001`, `var_final_002`（成功）、`var_final_reassign_001`, `var_final_compound_assign_001`, `var_final_array_001`（コンパイルエラー）

**説明**: 一度だけ代入可能で、その後は再代入不可の変数。
constexpr とは異なり、ランタイム値を保持できる実体のある変数。

### 3.1 構文

```nospace
func: __main() {
  final: x(10);    # 初期値付き（推奨）#
  # x = 20;        # コンパイルエラー: final 変数への再代入 #
  __assert(x == 10);

  final: y;        # 初期値なし（1回だけ代入可能）#
  y = compute();   # OK: 初回代入 #
  # y = 30;        # コンパイルエラー: 2回目の代入 #
}
```

### 3.2 セマンティクス

- `final: name(expr)` で再代入不可の変数を定義（スタックスロットを確保）
- 初期値が指定された場合、以降の代入はすべてコンパイルエラー
- 初期値なしの場合、1回だけ代入可能（以降はコンパイルエラー）
  - ただし、初回代入の検証は静的解析の範囲内で行う
  - すべてのパスで正確に1回代入されることの保証は複雑なため、初回実装では簡易チェックのみ
- `&final_var` は可能（アドレスが存在する）
- ランタイム値を保持可能（関数呼び出しの結果など）

### 3.3 実装に必要な変更

1. **トークンパーサ**: `final` キーワードの追加
2. **構文解析器**: `Statement::VariableDeclaration` に mutability フラグ追加
3. **意味解析器**:
   - `Variable` 構造体に `is_final: bool` フィールドを追加
   - 代入文（`Operator2::Assign`）でターゲットが final 変数の場合にエラー
4. **コンパイラ・インタプリタ**: 変更不要（final は意味解析でのみチェック）

---

## 4. 実装計画

### 4.1 ステップ一覧

| Step | 内容 | 依存 | 状態 |
|------|------|------|------|
| 1 | 純粋演算評価の共有モジュール (`base/pure_eval`) | なし | ✅ 実装済み |
| 2 | constexpr（式形式） | Step 1 | ✅ 実装済み |
| 3 | alias（識別子エイリアス） | なし | ✅ 実装済み |
| 4 | alias（ブロックエイリアス） | Step 3 | ✅ 実装済み |
| 5 | final 変数 | なし | ✅ 実装済み |
| 6 | constexpr ブロック形式 | Step 2 | ❌ 未設計 |
| 7 | spec.md / grammar.bnf への反映 | Step 2–5 | ✅ 実装済み |

**依存関係**:
```
Step 1 → Step 2 → Step 6
Step 3 → Step 4
Step 2–5 → Step 7
```

Step 1 と Step 3 と Step 5 は互いに独立しており、並行して実装可能。

---

### 4.2 Step 1: 純粋演算評価の共有モジュール

**目的**: constexpr 評価器・interpreter・optimizer で重複する算術/比較/論理演算の評価ロジックを共通化する。

#### 背景: 現状の重複

以下の3箇所で同一の演算評価ロジックが必要になる:

| モジュール | 用途 | 現状 |
|-----------|------|------|
| `src/interpreter/exec.rs` | ランタイム式評価 | `interpret_operation1` / `interpret_operation2` 内の match ブロック |
| `src/optimizer/constant_folding.rs` | コンパイル時最適化 | `try_fold_op1` / `try_fold_op2` 内の match ブロック |
| `src/semantic_analyzer/` (新規) | constexpr 定数式評価 | `evaluate_constexpr()` （未実装） |

**重複する演算**:
- 二項演算: `Plus`, `Minus`, `Multiply`, `Divide`, `Modulo`, `Equal`, `NotEqual`, `Less`, `LessEqual`, `Greater`, `GreaterEqual`
- 単項演算: `Negative`, `LogicalNot`

**現在の不整合**:
- constant_folding は `wrapping_*` 系（オーバーフロー安全）を使用
- interpreter は通常演算子（オーバーフロー時にパニック）を使用
- 共有モジュール導入時にどちらに統一するか決定が必要

#### 設計: `src/base/pure_eval.rs`

`src/base/` に純粋演算評価モジュールを新設する。
`base` モジュールは全コンパイラパスから参照可能であり、
interpreter / optimizer / semantic_analyzer のいずれからも依存できる。

```rust
// src/base/pure_eval.rs

use crate::tree_parser::{Operator1, Operator2};

/// bool を nospace の整数表現（0/1）に変換する
pub fn bool_to_int(b: bool) -> i64 {
    if b { 1 } else { 0 }
}

/// 純粋な二項演算を評価する
///
/// 副作用を持つ演算（Assign 系）や短絡評価が必要な演算（LogicalAnd/Or）は
/// None を返す。0除算も None を返す。
pub fn eval_binary_pure(op: &Operator2, lhs: i64, rhs: i64) -> Option<i64> {
    match op {
        Operator2::Plus => Some(lhs.wrapping_add(rhs)),
        Operator2::Minus => Some(lhs.wrapping_sub(rhs)),
        Operator2::Multiply => Some(lhs.wrapping_mul(rhs)),
        Operator2::Divide => {
            if rhs != 0 { Some(lhs.wrapping_div(rhs)) } else { None }
        }
        Operator2::Modulo => {
            if rhs != 0 { Some(lhs.wrapping_rem(rhs)) } else { None }
        }
        Operator2::Equal => Some(bool_to_int(lhs == rhs)),
        Operator2::NotEqual => Some(bool_to_int(lhs != rhs)),
        Operator2::Less => Some(bool_to_int(lhs < rhs)),
        Operator2::LessEqual => Some(bool_to_int(lhs <= rhs)),
        Operator2::Greater => Some(bool_to_int(lhs > rhs)),
        Operator2::GreaterEqual => Some(bool_to_int(lhs >= rhs)),
        // Assign 系、LogicalAnd/Or は呼び出し元が個別に処理
        _ => None,
    }
}

/// 純粋な単項演算を評価する
///
/// Ref / Deref はランタイム操作のため None を返す。
pub fn eval_unary_pure(op: &Operator1, val: i64) -> Option<i64> {
    match op {
        Operator1::Negative => Some(val.wrapping_neg()),
        Operator1::LogicalNot => Some(bool_to_int(val == 0)),
        _ => None,
    }
}
```

#### 変更対象モジュール

**新規ファイル**:
- `src/base/pure_eval.rs`: 上記の共有関数を定義
- `src/base/mod.rs`: `pub mod pure_eval;` を追加

**interpreter/exec.rs**:
- `interpret_operation2` の純粋演算部分を `eval_binary_pure()` 呼び出しに置換
- `interpret_operation1` の `Negative` / `LogicalNot` を `eval_unary_pure()` 呼び出しに置換
- `bool_to_int` を `base::pure_eval::bool_to_int` からインポート（`types.rs` から削除）
- Assign, LogicalAnd/Or, Ref, Deref の処理はそのまま維持

**optimizer/constant_folding.rs**:
- `try_fold_op2` の演算 match を `eval_binary_pure()` 呼び出しに置換
- `try_fold_op1` の演算 match を `eval_unary_pure()` 呼び出しに置換

#### 依存関係の注意点

```
base/pure_eval ← tree_parser（Operator1, Operator2 の定義を参照）

interpreter     → base/pure_eval（ランタイム演算評価）
optimizer       → base/pure_eval（定数畳み込み）
semantic_analyzer → base/pure_eval（constexpr 評価）
```

`base` → `tree_parser` への依存が新たに発生する点に注意。
現在 `base` は `SourceLocation` のみを提供しており、他モジュールへの依存はない。
`Operator1` / `Operator2` は単純な enum であり、この依存は許容範囲と考える。
ただし、依存方向を逆転させたくない場合は、演算子 enum を `base` に移動する選択肢もある。

---

### 4.3 Step 2: constexpr（式形式）

**目的**: コンパイル時定数式 `constexpr: name(expr);` を実装する。
仕様の詳細は §2.1〜2.3, 2.5 を参照。

#### 変更対象モジュール

**token_parser**:

| 変更 | 内容 |
|------|------|
| `Keyword` enum に追加 | `Constexpr` |
| `as_keyword_token` に追加 | `"constexpr"` → `Keyword::Constexpr` |

**tree_parser/statement**:

| 変更 | 内容 |
|------|------|
| `Statement` enum に追加 | `ConstexprDeclaration(String, Box<LocatedExpression>)` — 定数定義 (name, expr) |
| パース処理追加 | `constexpr:` キーワード後の構文解析 |

パース規則:
```
"constexpr" ":" ident "(" expr ")" ("," ident "(" expr ")")* ";"   → ConstexprDeclaration (複数定義可)
```

**semantic_analyzer**:

| 変更 | 内容 |
|------|------|
| 新しいパス追加 | Pass 0: constexpr 定義の収集・評価 |
| `ScopeBuilder` に追加 | `constexpr_table: BTreeMap<String, i64>` |
| `ScopeResolver` に追加 | constexpr テーブル参照（変数参照時に定数値に置換） |
| 新規関数追加 | `evaluate_constexpr()` — `base::pure_eval` を利用した定数式評価器 |
| 巡回検知追加 | 訪問済みセットによる巡回参照チェック |

パス構成の変更（3パス → 4パス）:
```
Pass 0:  constexpr 定義の収集・評価（新規）
Pass 1a: 関数宣言のホイスティング（既存）
Pass 1b: 変数宣言のホイスティング（既存）
Pass 2:  文の変換・識別子解決（既存 + constexpr 解決）
```

**interpreter / compiler_ws**: 変更不要（constexpr は `Factor(value)` に置換済み）

**テスト**: constexpr の基本テストケースを追加

---

### 4.4 Step 3: alias（識別子エイリアス）

**目的**: 識別子エイリアス `alias: name(target);` を実装する。
仕様の詳細は §1.1, 1.3（巡回検知）, 1.4（ホイスティング）を参照。

#### 変更対象モジュール

**token_parser**:

| 変更 | 内容 |
|------|------|
| `Keyword` enum に追加 | `Alias` |
| `as_keyword_token` に追加 | `"alias"` → `Keyword::Alias` |

**tree_parser/statement**:

| 変更 | 内容 |
|------|------|
| `Statement` enum に追加 | `AliasIdentifier(String, String)` — 識別子エイリアス (name, target) |
| パース処理追加 | `alias:` + `ident "(" ident ")"` の構文解析 |

パース規則:
```
"alias" ":" ident "(" ident ")" ";"   → AliasIdentifier
```

**semantic_analyzer**:

| 変更 | 内容 |
|------|------|
| Pass 0 に追加 | alias 識別子定義の収集 |
| `ScopeBuilder` に追加 | `alias_map: BTreeMap<String, AliasEntry>` |
| `ScopeResolver` に追加 | alias チェーン解決ロジック |
| 新規関数追加 | `resolve_alias_chain()` — 訪問済みセットによる巡回検知付き |

**interpreter / compiler_ws**: 変更不要（alias は名前解決時に IdentifierRef に変換済み）

**テスト**: 識別子エイリアスの基本テストケースを追加

---

### 4.5 Step 4: alias（ブロックエイリアス）

**目的**: ブロックエイリアス `alias: name { 文... };` を実装する。
仕様の詳細は §1.2, 1.3（巡回検知）を参照。

**前提**: Step 3 の alias 識別子エイリアスが実装済みであること。

#### 変更対象モジュール

**tree_parser/statement**:

| 変更 | 内容 |
|------|------|
| `Statement` enum に追加 | `AliasBlock(String, Vec<LocatedStatement>)` — ブロックエイリアス (name, block) |
| パース処理追加 | `alias:` + `ident block ";"` の構文解析 |

パース規則:
```
"alias" ":" ident block ";"   → AliasBlock
```

**semantic_analyzer**:

| 変更 | 内容 |
|------|------|
| Pass 0 に追加 | alias ブロック定義の収集（AST を保存） |
| 式変換に追加 | `name()` 呼び出し時に AST をクローンして展開 |
| 巡回検知追加 | 展開スタックによる再帰的展開チェック |

**interpreter / compiler_ws**: 変更不要（ブロックエイリアスはブロック式に展開済み）

**テスト**: ブロックエイリアスの基本テスト、巡回参照エラーテストを追加

---

### 4.6 Step 5: final 変数

**目的**: 再代入不可の変数 `final: name(expr);` を実装する。
仕様の詳細は §3 を参照。

#### 変更対象モジュール

**token_parser**:

| 変更 | 内容 |
|------|------|
| `Keyword` enum に追加 | `Final` |
| `as_keyword_token` に追加 | `"final"` → `Keyword::Final` |

**tree_parser/statement**:

| 変更 | 内容 |
|------|------|
| `Statement::VariableDeclaration` に追加 | mutability フラグ（`is_final: bool`）|

**semantic_analyzer**:

| 変更 | 内容 |
|------|------|
| `Variable` 構造体に追加 | `is_final: bool` フィールド |
| 代入チェック追加 | `Operator2::Assign` で final 変数へのターゲットをエラー |

**interpreter / compiler_ws**: 変更不要（final は意味解析でのみチェック）

**テスト**: final 変数の基本テスト、再代入エラーテストを追加

---

### 4.7 Step 6: constexpr ブロック形式（将来拡張）

**目的**: ブロック形式の constexpr を設計・実装する。
仕様の概要は §2.4 を参照。

**状態**: ❌ 未設計

**前提**: Step 2 の constexpr（式形式）が実装済みであること。

このステップでは以下を検討する:
- ブロック内で許可される文の種類（let, if, while 等）
- constexpr ブロック内の変数スコープの扱い
- `evaluate_constexpr()` のブロック対応拡張
- 既存の `constant_folding` との関係（ブロック畳み込みとの統合可能性）

設計は Step 2 の実装経験を踏まえて行う。

---

### 4.8 Step 7: spec.md / grammar.bnf への反映

**目的**: 実装完了した機能を言語仕様ドキュメントに反映する。

**前提**: Step 2〜5 のうち、反映対象の機能が実装済みであること。

更新対象:
- `docs/spec.md`「代入・変数定義」セクション: constexpr / alias の構文・セマンティクスを追加
- `docs/spec.md`「スコープ」セクション: alias のスコープルールを追加
- `docs/grammar.bnf`: alias / constexpr / final の文法規則を追加

---

## 5. 設計上の未決定事項

1. **ブロックエイリアスの展開制限**: 無制限に展開を許可するとコンパイル時間が爆発する可能性がある。展開深度の上限を設けるか？
2. **constexpr の型**: 現在は int のみだが、将来の型システム拡張時にどう扱うか
3. **alias のシャドウイング**: 子スコープで同名の alias を再定義できるか？（let/func と同じルールに合わせるのが自然）
4. **ブロックエイリアスとスコープキャプチャ**: 現在の設計はマクロ的展開（呼び出し元スコープ）だが、クロージャ的（定義元スコープ）の方が安全性が高い。どちらを採用するか要検討

---

## 関連ドキュメント

- [docs/spec.md](../../docs/spec.md) - 言語仕様
- [docs/grammar.bnf](../../docs/grammar.bnf) - 文法定義
- [ai-docs/done-task/block-scope-global-variables-implementation.md](../done-task/block-scope-global-variables-implementation.md) - 実装済みの変数機能
- [ai-docs/done-task/implement-multi-variable-declaration.md](../done-task/implement-multi-variable-declaration.md) - 実装済みの変数初期化機能
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md) - 実装状況の詳細
- テスト: [disabled_var_final_001.ns](../../resources/tests/passes/variables/disabled_var_final_001.ns)

---

## 更新履歴

- 2026-02-28: Step 7 完了。`docs/spec.md` の constexpr・final・alias 各セクションを更新、`docs/grammar.bnf` に alias/constexpr/final 文法規則を追加
- 2026-02-28: Step 5 完了。`final:` 変数の実装、コンパイル時再代入チェック追加
- 2026-02-28: Step 4 完了。ブロックエイリアス `alias: name { stmts }` の実装
- 2026-02-28: Step 1 実装完了。`src/base/pure_eval.rs` を新規作成し、`interpreter/exec.rs` と `optimizer/constant_folding.rs` を更新
- 2026-02-28: alias 設計・const 再設計を追加。const を変数からコンパイル時定数エイリアスに変更
- 2026-02-28: const → constexpr にリネーム。純粋演算評価の共有モジュール設計を追加
- 2026-02-28: 実装計画をステップごとに分割（Step 1〜7）
- 2026-02-10: 変数初期化機能が実装済みのため、該当セクションを削除
- 2026-02-07: unimplemented-features.md から分離して作成
