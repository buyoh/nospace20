# 未実装の変数関連機能

このドキュメントは nospace プログラミング言語における未実装の変数関連機能をまとめたものです。

最終更新日: 2026-02-28

## 目次

1. [alias（エイリアス）](#1-aliasエイリアス)
2. [const（コンパイル時定数エイリアス）](#2-constコンパイル時定数エイリアス)
3. [final 変数](#3-final-変数)
4. [実装計画](#4-実装計画)

---

## 1. alias（エイリアス）

**状態**: ❌ 未実装

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

## 2. const（コンパイル時定数エイリアス）

**状態**: ❌ 未実装（旧設計から変更）

**旧設計との違い**:
- 旧設計: `const` は再代入不可の変数（スタックスロットを確保）
- 新設計: `const` はコンパイル時に解決される定数エイリアス（変数ではない）
- `const` は `alias` の制約付きバージョンとして位置づけられる

### 2.1 構文

```nospace
const: PI(3);                  # リテラル定数 #
const: SIZE(2 + 3);            # コンパイル時式（= 5 に解決）#
const: DOUBLE_PI(PI * 2);      # 他の const を参照可能 #
# const: X(variable);          # コンパイルエラー: 非定数式 #
# const: Y(func1());           # コンパイルエラー: 関数呼び出しは非定数 #
```

### 2.2 セマンティクス

- `const: name(expr)` で `name` をコンパイル時定数として定義
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
| 他の const 参照 | ✅ | 解決済みの値を使用 |
| 算術演算 (+, -, *, /, %) | ✅ | オペランドが定数式の場合 |
| 比較演算 (==, !=, <, <=, >, >=) | ✅ | オペランドが定数式の場合 |
| 論理演算 (&&, \|\|, !) | ✅ | オペランドが定数式の場合 |
| 変数参照 | ❌ | ランタイム値 |
| 関数呼び出し | ❌ | 副作用の可能性 |
| if / ブロック | ❌ | 制御フローは非定数 |

**評価器の実装**:

```
evaluate_constexpr(expr, const_table) -> Result<i64, Error>:
  match expr:
    Factor(n) → Ok(n)
    Variable(name):
      if name ∈ const_table:
        Ok(const_table[name])
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

### 2.4 const ブロック形式（将来拡張）

```nospace
# 将来的に検討 #
const: VALUE {
  let: tmp(3);
  tmp * tmp;   # = 9 #
};
```

ブロック内の全ての処理がコンパイル時に評価可能な場合のみ許可。
初回実装では式形式のみをサポートし、ブロック形式は将来拡張とする。

### 2.5 const のホイスティングと定義順序

- const はスコープ内でホイスティングされる
- ただし、const の初期化式内で別の const を参照する場合、
  参照先の const が先に定義されている（or 同一スコープ内で定義されている）必要がある
- 巡回参照はコンパイルエラー

```nospace
const: A(B + 1);  # OK: B は同一スコープ内で定義 #
const: B(10);

# const: X(Y + 1);  巡回参照エラー #
# const: Y(X + 1);  #
```

---

## 3. final 変数

**状態**: ❌ 未実装

**説明**: 一度だけ代入可能で、その後は再代入不可の変数。
const とは異なり、ランタイム値を保持できる実体のある変数。

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

### 4.1 モジュールごとの変更一覧

#### token_parser

| 変更 | 内容 |
|------|------|
| `Keyword` enum に追加 | `Alias`, `Const` (final は別タスク) |
| `as_keyword_token` に追加 | `"alias"` → `Keyword::Alias`, `"const"` → `Keyword::Const` |

#### tree_parser/statement

| 変更 | 内容 |
|------|------|
| `Statement` enum に追加 | `AliasIdentifier(String, String)` — 識別子エイリアス (name, target) |
| `Statement` enum に追加 | `AliasBlock(String, Vec<LocatedStatement>)` — ブロックエイリアス (name, block) |
| `Statement` enum に追加 | `ConstDeclaration(String, Box<LocatedExpression>)` — 定数定義 (name, expr) |
| パース処理追加 | `alias:` キーワード後の構文解析 |
| パース処理追加 | `const:` キーワード後の構文解析 |

**alias 構文パース**:
```
"alias" ":" ident "(" ident ")" ";"              → AliasIdentifier
"alias" ":" ident block ";"                       → AliasBlock
```

**const 構文パース**:
```
"const" ":" ident "(" expr ")" ("," ident "(" expr ")")* ";"   → ConstDeclaration (複数定義可)
```

#### semantic_analyzer

| 変更 | 内容 |
|------|------|
| 新しいパス追加 | Pass 0: alias / const 定義の収集 |
| `ScopeBuilder` に追加 | `alias_map: BTreeMap<String, AliasEntry>` |
| `ScopeBuilder` に追加 | `const_table: BTreeMap<String, i64>` |
| `ScopeResolver` に追加 | alias チェーン解決ロジック |
| `ScopeResolver` に追加 | const テーブル参照 |
| 新規関数追加 | `evaluate_constexpr()` — 定数式評価器 |
| 新規関数追加 | `resolve_alias_chain()` — エイリアスチェーン解決 |
| 巡回検知追加 | 展開スタック / 訪問済みセットの管理 |

**3パス → 4パス解析**:
```
Pass 0:  alias / const 定義の収集・評価
Pass 1a: 関数宣言のホイスティング（既存）
Pass 1b: 変数宣言のホイスティング（既存）
Pass 2:  文の変換・識別子解決（既存 + alias/const 解決）
```

#### interpreter / compiler_ws

- **変更不要**: alias と const はコンパイル時に完全に解決されるため、
  実行時の中間表現（ExecExpression / ExecStatement）には影響しない
- alias → 名前解決の結果として IdentifierRef / ブロック式に変換済み
- const → `Factor(value)` に置換済み

### 4.2 実装優先順位

1. **const** — 定数式評価器は比較的単純で、既存の定数畳み込みを流用可能
2. **alias（識別子）** — 名前解決テーブルへの追加で実現可能
3. **alias（ブロック）** — AST クローン・展開のロジックが必要でやや複雑
4. **final** — 代入チェックの実装が必要（別タスクとして実装してもよい）

### 4.3 spec.md への反映

実装後、以下のセクションを更新する必要がある:
- 「代入・変数定義」セクション: const / alias の構文・セマンティクスを追加
- 「スコープ」セクション: alias のスコープルールを追加
- grammar.bnf: alias / const の文法規則を追加

---

## 5. 設計上の未決定事項

1. **ブロックエイリアスの展開制限**: 無制限に展開を許可するとコンパイル時間が爆発する可能性がある。展開深度の上限を設けるか？
2. **const の型**: 現在は int のみだが、将来の型システム拡張時にどう扱うか
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

- 2026-02-28: alias 設計・const 再設計を追加。const を変数からコンパイル時定数エイリアスに変更
- 2026-02-10: 変数初期化機能が実装済みのため、該当セクションを削除
- 2026-02-07: unimplemented-features.md から分離して作成
