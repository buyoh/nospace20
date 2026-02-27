# Phase 5: 文字列リテラル（糖衣構文）の実装

## 概要

docs/spec.md §4.3 の文字列リテラルを実装する。
文字列は配列宣言の糖衣構文であり、tree_parser レベルで配列に変換する。

## 仕様

```nospace
let: str1("Hello");
# ↓ 以下と同等
let: str2[6]('H', 'e', 'l', 'l', 'o', '\0');
```

- 文字列リテラルは `"..."` で囲む
- 末尾は自動的にヌル文字 (`\0`) で終端
- 文字列の中でも空白文字は無視されるため、エスケープ (`\s`) を使用
- サイズは文字数 + 1（ヌル終端分）

## 依存関係

- Phase 1〜3 が完了済みであること（配列の基本機能が動作すること）
- token_parser に文字列リテラルの字句解析を追加する必要がある

## 変更ファイル

- `src/token_parser/mod.rs` — 文字列リテラルトークンの追加
- `src/tree_parser/statement/mod.rs` — 糖衣構文の展開

## 1. Token の拡張

### 新規トークン

```rust
pub enum Token {
    // ... 既存のトークン ...
    StringLiteral(Vec<i64>),  // 文字列リテラル（各文字のASCII値のベクタ）
}
```

**あるいは**、より単純に文字列リテラルを `"..."` としてパースし、
token_parser で各文字の ASCII 値に変換する。

`StringLiteral` は既にヌル終端を含まない生の文字値列とする。
ヌル終端は tree_parser で追加する。

### 字句解析

```rust
'"' => {
    let mut chars = Vec::new();
    loop {
        match /* next char */ {
            '"' => break,
            '\\' => {
                // エスケープシーケンスの処理（文字リテラルと同じ）
                chars.push(parse_escape_sequence());
            }
            c if c.is_whitespace() => continue, // 空白は無視
            c => chars.push(c as i64),
        }
    }
    Token::StringLiteral(chars)
}
```

**注意**: nospace では空白文字が意味を持たない（無視される）ため、
文字列中の空白も無視される。`"Hello World"` は `"HelloWorld"` と同じ。
スペースを含めるには `"Hello\sWorld"` と書く。

## 2. tree_parser での展開

`parse_variable_declarations` で初期化式を処理する際:

```nospace
let: str("Hello");
```

1. `let:` → 変数宣言開始
2. `str` → 識別子
3. `(` → 初期化式開始
4. `"Hello"` → `StringLiteral(['H', 'e', 'l', 'l', 'o'])`
5. `)` → 初期化式終了

**展開ルール**:
- 初期化式が `StringLiteral` 1つだけの場合、配列宣言として展開
- 配列サイズ = 文字数 + 1（ヌル終端分）
- 生成:
  - `VariableDeclaration("str", Factor(0), false, Some(6))`
  - `Expression(str[0] = 'H')`
  - `Expression(str[1] = 'e')`
  - `Expression(str[2] = 'l')`
  - `Expression(str[3] = 'l')`
  - `Expression(str[4] = 'o')`
  - `Expression(str[5] = '\0')`

### 明示的サイズ指定との組み合わせ

`let: str[10]("Hello");` の場合:
- 配列サイズは明示的に 10
- 文字列 "Hello" + ヌル終端 = 6 文字
- 残りの 4 スロットは 0 初期化（デフォルト）
- エラーチェック: 文字列長 + 1 > 配列サイズ の場合はエラー

## 3. テスト項目

- `let: s("Hello"); assert(s[0] == 'H'); assert(s[5] == 0);`
- `let: s("A\sB"); assert(s[0] == 'A'); assert(s[1] == 32); assert(s[2] == 'B');`
- エスケープ: `let: s("\n\t\\"); assert(s[0] == 10); assert(s[1] == 9); assert(s[2] == 92);`
- 空文字列: `let: s(""); assert(s[0] == 0);` → サイズ 1 の配列
- サイズ指定: `let: s[10]("Hi"); assert(s[2] == 0);` → 有効
- エラー: `let: s[2]("Hello");` → サイズ不足エラー

## 4. 考慮事項

### token_parser の変更範囲

文字列リテラル用のダブルクォートパースを追加する必要がある。
ただし、nospace は空白を無視する言語であるため、`"` の中でも空白は意味を持たない。
これは spec に記載の通り。

### Token::StringLiteral の設計判断

**案A**: `StringLiteral(Vec<i64>)` — パース時に各文字をASCII値に変換

**案B**: `StringLiteral(String)` — 文字列として保持し、tree_parser で変換

**採用**: 案A。token_parser で既に文字リテラル（`'A'` → `Number(65)`）の変換があるため、
同じ仕組みで文字列中の各文字を変換する。

### 初期化式が文字列のみか判定

`let: str("Hello");` vs `let: x(5);` の区別:
- `(` の後の最初のトークンが `StringLiteral` → 文字列初期化
- それ以外 → 通常の式初期化

あるいは、tree_parser ではなく、statement レベルで:
- 配列サイズが指定されていない + 初期化式が `StringLiteral` → 暗黙的配列宣言

この場合、`Statement::VariableDeclaration` の `array_size` は tree_parser が計算して設定する。
