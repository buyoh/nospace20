# モジュール詳細

## base

**ファイル**: `src/base/mod.rs`

共通の型定義を提供するモジュール。

### 主要な型

| 型 | 説明 |
|----|------|
| `CodeParseErrorInternal` | 内部使用のパースエラー (デバッグ情報含む) |
| `CodeParseError` | 外部公開用のパースエラー |

### マクロ

- `code_parse_error!` - エラー生成マクロ

---

## token_parser

**ファイル**: `src/token_parser/mod.rs`, `src/token_parser/test.rs`

字句解析器。文字列をトークン列に変換する。

### 主要な型

| 型 | 説明 |
|----|------|
| `Token` | トークンの種類 (列挙型) |
| `Keyword` | キーワード (let, func, if, else, while, return, break, continue) |
| `TokenInfo` | トークンの位置情報 |
| `PrettyToken` | `(Token, TokenInfo)` のタプル |

### 対応トークン

- **リテラル**: 数値 (`Number`)
- **識別子**: `Identifier`
- **キーワード**: `let`, `func`, `if`, `else`, `while`, `return`, `break`, `continue`
- **演算子**: `+`, `-`, `*`, `/`, `!`, `=`, `==`, `!=`, `<`, `>`, `<=`, `>=`
- **括弧**: `()`, `[]`, `{}`
- **区切り**: `;`, `:`, `,`

### コメント

`#` で囲まれた部分がコメントとして扱われる。

```
# これはコメント #
```

---

## tree_parser

**ファイル**: `src/tree_parser/mod.rs`, `expression.rs`, `statement.rs`

構文解析器。トークン列を抽象構文木に変換する。

### 主要な型

| 型 | 説明 |
|----|------|
| `Statement` | 文 (変数宣言、関数宣言、return、式文等) |
| `Expression` | 式 (演算、if、関数呼び出し、リテラル、変数参照) |
| `Operator1` | 単項演算子 |
| `Operator2` | 二項演算子 |

### Statement の種類

```rust
pub enum Statement {
    VariableDeclaration(String, Box<Expression>),  // let: x;
    FunctionDeclaration(String, Vec<String>, Vec<Statement>),  // func: name(args) { ... }
    Continue,
    Break,
    Return(Box<Expression>),  // return: expr;
    While(Box<Expression>, Vec<Statement>),  // while: cond { ... };
    Expression(Box<Expression>),  // 式文
    Invalid(usize),
}
```

### Expression の種類

```rust
pub enum Expression {
    Operation1(Operator1, Box<Expression>),
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    If(Box<Expression>, Vec<Statement>, Vec<Statement>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
    Invalid(usize),
}
```

---

## semantic_analyzer

**ファイル**: `src/semantic_analyzer/mod.rs`

意味解析器。ASTを実行可能な構造に変換する。

### 主要な型

| 型 | 説明 |
|----|------|
| `Scope` | スコープ (変数・関数の管理) |
| `Function` | 関数定義 (引数、スコープ、コード) |
| `Variable` | 変数定義 |
| `ExecStatement` | 実行可能な文 |
| `ExecExpression` | 実行可能な式 |

### Scope 構造

```rust
pub struct Scope {
    identifier_map: BTreeMap<String, Identifier>,
    pub variables: Vec<Variable>,
    functions: Vec<Function>,
}
```

メソッド:
- `get_function(&self, id: &str) -> Option<&Function>`
- `get_variable(&self, id: &str) -> Option<&Variable>`

---

## interpreter

**ファイル**: `src/interpreter/mod.rs`

インタプリタ。構文解析済みのコードを実行する。

### 主要な型

| 型 | 説明 |
|----|------|
| `Environment` | グローバル実行環境 (トレース情報等) |
| `LocalEnvironment` | ローカル実行環境 (変数値等) |
| `Flow` | 制御フロー (Proceed, Return, Continue, Break) |
| `ExpressionFlow` | 式の評価結果 (Value, Jump) |

### 組み込み関数

| 関数 | 説明 |
|------|------|
| `__clog(x)` | 値をコンソールに出力 |
| `__assert(x)` | `x == 0` でパニック |
| `__assert_not(x)` | `x != 0` でパニック |
| `__trace(key)` | テスト用トレースポイント記録 |
| `__puti(x)` | 整数を10進数で標準出力に出力 |
| `__putc(x)` | 文字（ASCII値）を標準出力に出力 |
| `__geti()` | 標準入力から整数を読み込み |
| `__getc()` | 標準入力から1文字を読み込み |

---

## compiler

**ファイル**: `src/compiler/mod.rs`

コンパイラ。**未実装**。

`grayspace/` サブディレクトリが存在するが、空の状態。

---

## optimizer

**ファイル**: `src/optimizer/mod.rs`

意味解析後の中間表現 (`Scope`) に対して最適化パスを適用するモジュール。

### 主要な型

| 型 | 説明 |
|----|------|
| `OptimizationOptions` | 各最適化パスの有効化・無効化を制御する設定 |

### パス一覧

| パス | ファイル | 説明 |
|------|----------|------|
| `noop_test_pass` | `noop_test_pass.rs` | フレームワーク動作検証用ダミーパス |

### 公開 API

- `optimize(scope: &mut Scope, options: &OptimizationOptions)` - 最適化パスを適用

---

## logger

**ファイル**: `src/logger/mod.rs`

テキスト処理ユーティリティ。

### 主要な型

| 型 | 説明 |
|----|------|
| `TextCode` | ソースコードのテキスト管理 |

### TextCode メソッド

- `new(source: &str)` - 初期化
- `line(i: usize)` - 指定行を取得
- `char_index_to_line(i: usize)` - 文字インデックスから (行, 列) へ変換
