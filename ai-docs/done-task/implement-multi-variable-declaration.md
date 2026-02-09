# 複数変数宣言・初期化宣言の実装

## 概要

`let:` 文の2つの未実装機能を設計・実装する:

1. **複数変数同時宣言**: `let: a, b;`
2. **初期化付き宣言**: `let: x(5);`

### 仕様 (spec.md §4, §4.1)

```
let: z;              # z を 0 で初期化 #
let: u, v;           # 複数変数を宣言 #
let: x(5);           # x を 5 で初期化 #
let: a(1), b(2);     # 複数変数を初期化して宣言 #
```

### 初期化の動作仕様

変数はホイスティングにより **スコープ先頭で 0 で初期化** される。
初期化式 `(expr)` は **宣言された位置で評価し代入** される。

```
__clog(a);     # 0 (ホイスティングで 0 初期化済み) #
a = 5;         # ホイスティングにより有効 #
__clog(a);     # 5 #
let: a(3);     # この位置で a = 3 を実行 #
__clog(a);     # 3 #
```

### 失敗しているテストケース

- `test_legacy_015`: `let:r1, r2;`
- `test_legacy_020`: `let:n,x;`
- `test_legacy_023`: `let:a,b;`

---

## 現在のデータフロー

```
ソースコード: let: x;
  ↓ token_parser
トークン列: [Keyword(Let), Colon, Identifier("x"), Semicolon]
  ↓ tree_parser
AST: Statement::VariableDeclaration("x", Expression::Factor(0), false)
  ↓ semantic_analyzer パス1 (ホイスティング)
Scope に Variable { identifier: "x", is_static: false } を登録
  ↓ semantic_analyzer パス2 (文変換)
ExecStatement::Expression(ExecExpression::Factor(0))  ← 初期化式のみ残る
  ↓ interpreter
0を評価して破棄。変数領域はスコープ進入時に vec![0; N] で確保済み
  ↓ compiler_ws
Push(0) + Discard を生成。変数領域はヒープに確保済み
```

---

## 設計

### 基本方針: パーサーレベルで展開

複数変数宣言と初期化宣言はパーサーで処理し、**セマンティック解析・インタプリタ・コンパイラへの変更を不要にする**。

- `let: a, b(5);` → 複数の `Statement::VariableDeclaration` に展開
- AST の `Statement::VariableDeclaration(String, Box<Expression>, bool)` の構造は変更しない
- `static:` も同様に拡張する

### BNF 変更

```bnf
# 変更前
let ::= "let" ":" let_decl ";"
let_decl ::= ident

# 変更後
let ::= "let" ":" let_decl ("," let_decl)* ";"
let_decl ::= ident ("(" expr ")")?
```

---

### モジュール別設計

#### 1. tree_parser (パーサー) — 変更あり

**ファイル**: `src/tree_parser/statement/mod.rs`

##### 1.1 `parse_to_statements_let` の変更

**戻り値**: `LocatedStatement` → `Vec<LocatedStatement>`

**処理フロー**:
```
1. "let" を消費
2. ":" を消費
3. results = []
4. ループ:
   a. 識別子 name を消費
   b. 次のトークンを peek:
      - "(" の場合:
        i.   "(" を消費
        ii.  parse_expression() で初期化式 init_expr を取得
        iii. ")" を消費
        iv.  代入式を構築: init = Operation2(Assign, Variable(name), init_expr)
      - "(" でない場合:
        v.   init = Factor(0)
   c. results に VariableDeclaration(name, init, false) を追加
   d. 次のトークンを peek:
      - "," の場合: "," を消費して続行
      - それ以外: ループ終了
5. ";" を消費
6. results を返す
```

**呼び出し元の変更**: `parse_to_statements` 内で:
```rust
// 変更前: 単一の LocatedStatement を push
statements.push(self.parse_to_statements_let(start_pos));

// 変更後: Vec<LocatedStatement> を extend
statements.extend(self.parse_to_statements_let(start_pos));
```

##### 1.2 初期化式の代入式への変換

初期化式がある場合、パーサーで **代入式** を生成する:

```
let: a(3);
→ VariableDeclaration("a", Operation2(Assign, Variable("a"), Factor(3)), false)
```

これにより:
- セマンティック解析 パス1: 変数名 "a" を取得してホイスティング（0初期化で領域確保）
- セマンティック解析 パス2: 代入式 `a = 3` を `ExecStatement::Expression` に変換
- 実行時: 宣言位置で `a = 3` が評価される

初期化式がない場合:
```
let: a;
→ VariableDeclaration("a", Factor(0), false)
```
- パス2: `0` を評価して破棄（既存動作と同一、無害）

##### 1.3 `parse_to_statements_static` の変更

`parse_to_statements_let` と同様のカンマ区切り・初期化対応を適用。
`is_static = true` を使用する点のみ異なる。

コード重複を避けるため、共通の内部関数 `parse_variable_declarations(is_static: bool)` に統合することも検討。

##### 1.4 カンマとの競合について

初期化式内にカンマが出現するリスクについて:
- nospace の式文法にはカンマ演算子が存在しない
- `Token::Comma` は演算子として解釈されず、`parse_expression` はカンマで停止する
- したがって `let: a(1+2), b(3);` のような記述は安全にパースできる

#### 2. semantic_analyzer — 変更なし

パーサーが複数の `Statement::VariableDeclaration` を生成するため、既存の2パス処理がそのまま動作する:

- **パス1 (ホイスティング)**: 各 `VariableDeclaration` から変数名を取得しスコープに登録
  - 変数領域は実行時に 0 初期化される（`vec![0; variable_count]`）
- **パス2 (文変換)**: 各 `VariableDeclaration` の init_expr を `ExecStatement::Expression` に変換
  - 初期化式なし: `Factor(0)` → 0 を評価して破棄（無害）
  - 初期化式あり: `Operation2(Assign, Variable(name), init_expr)` → 代入式として実行

#### 3. interpreter — 変更なし

- 変数領域は `enter_block` / `new_func` で `vec![0; scope.variable_count]` として確保済み
- 初期化式は通常の式文 (`ExecStatement::Expression`) として実行される

#### 4. compiler_ws — 変更なし

- 変数領域は `generate_function_definition` でヒープ上に確保済み
- 初期化式は通常の式文として `generate_statement` でコード生成される

---

## 実装ステップ

### ステップ 1: パーサーの修正

1. `parse_to_statements_let` の戻り値を `Vec<LocatedStatement>` に変更
2. カンマ区切りのループ処理を追加
3. `(expr)` の初期化式パースを追加（代入式として構築）
4. 呼び出し元で `extend` に変更
5. `parse_to_statements_static` にも同様の修正を適用

### ステップ 2: ドキュメント更新

- `docs/grammar.bnf` の `let_decl` コメントを更新
- `spec.md` §4.1 の「(未実装)」表記を除去

### ステップ 3: テスト

- 既存テスト `test_legacy_015`, `test_legacy_020`, `test_legacy_023` がパスすることを確認
- 新規テストケース:
  - 単一変数初期化: `let: x(5);` → x == 5
  - 複数変数初期化: `let: a(1), b(2);` → a == 1, b == 2
  - ホイスティング + 初期化: spec のサンプルコード通りの動作
  - static 変数初期化: `static: a(5);` → 同様に動作
  - 初期化式中の計算: `let: x(2+3);` → x == 5

---

## スコープ外

以下は本タスクの範囲外:

- 配列宣言 (`let: arr[4];`)
- 文字列宣言 (`let: str("Hello");`)
- `final` / `const` 修飾子
