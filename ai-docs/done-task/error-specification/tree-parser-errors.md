# 構文解析エラー (Tree Parser Errors)

## 概要

構文解析フェーズでは、トークン列を構文木（AST）に変換する。このフェーズで検出されるエラーは、予期しないトークンの出現や欠落、不正な構文構造に関するものである。

**実装場所**: 
- `src/tree_parser/statement/mod.rs`
- `src/tree_parser/expression/mod.rs`

**エラー型**: `CodeParseError`

## エラー特性

構文解析エラーには2種類の処理方針がある：

1. **致命的エラー**: パースを中断し、`Expression::Invalid` または `Statement::Invalid` を返す
2. **弱いエラー**: エラーを記録するが、パースを継続する

## エラー一覧

### 1. 式のパースエラー

#### 1.1 予期しない入力終端

**エラーメッセージ**:
```
unexpected end of input
```

**発生条件**: 式の途中でトークン列が終了

**テストケース**: `resources/tests/fails/syntax/unexpected_eof_001.ns`

**ソースコード**: `src/tree_parser/expression/mod.rs:89`

```rust
return Box::new(Expression::Invalid(
    self.add_end_error("unexpected end of input"),
))
```

**例**:
```nospace
func: main() {
  let: x;
  x = (1 +   # エラー: 式が完結していない
}
```

---

#### 1.2 関数呼び出しの予期しないカンマ

**エラーメッセージ**:
```
unexpected comma
```

**発生条件**: 関数呼び出しの引数リストで不適切な位置にカンマがある

**ソースコード**: `src/tree_parser/expression/mod.rs:107-109`

```rust
if let State::Comma = state {
    // weak syntax error and proceed parsing
    self.add_parse_error(token_info, "unexpected comma");
}
```

**例**:
```nospace
func: main() {
  foo(1, , 2);  # エラー: unexpected comma
}
```

---

#### 1.3 関数呼び出しのカンマ欠落

**エラーメッセージ**:
```
missing comma
```

**発生条件**: 関数呼び出しの引数間にカンマがない

**ソースコード**: `src/tree_parser/expression/mod.rs:120-123`

```rust
if let State::Eval = state {
    // weak syntax error and proceed parsing
    self.add_parse_error(token_info, "missing comma");
}
```

**例**:
```nospace
func: main() {
  foo(1 2 3);  # エラー: missing comma
}
```

---

#### 1.4 括弧が閉じられていない

**エラーメッセージ**: マクロ展開により生成される

**発生条件**: 開き括弧に対応する閉じ括弧がない

**テストケース**: `resources/tests/fails/syntax/unclosed_paren_001.ns`

**ソースコード**: `src/tree_parser/expression/mod.rs` (マクロ使用箇所)

**例**:
```nospace
func: main() {
  let: x;
  x = (1 + 2;  # エラー: 閉じ括弧がない
}
```

---

### 2. 文のパースエラー

#### 2.1 予期しないトークン

**エラーメッセージ**: マクロ展開により生成される（期待されるトークンによる）

**発生条件**: 構文的に期待されるトークンと異なるトークンが出現

**ソースコード**: `src/tree_parser/macros.rs` のマクロ定義

主要マクロ：
- `match_expect_token!` - 期待されるトークンと一致しない場合にエラー
- `match_expect_token_unused!` - エラーを報告するが値を使用しない

**例**:
```nospace
func: main() {
  let x;      # エラー: let の後にコロンが必要
  func main   # エラー: func の後にコロンが必要
}
```

---

#### 2.2 識別子の欠落

**エラーメッセージ**: （期待される識別子の文脈により異なる）

**発生条件**: 変数宣言や関数宣言で識別子が期待される位置に別のトークンがある

**ソースコード**: `src/tree_parser/statement/mod.rs:77-84`

```rust
let id = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
    Ok(x) => x,
    Err(e) => {
        return LocatedStatement {
            statement: Statement::Invalid(e),
            location: SourceLocation::from_single(start_pos),
        };
    }
};
```

---

### 3. パース継続方針

構文解析エラーには以下の2つの処理方針がある：

#### 3.1 致命的エラー（パース中断）

以下の場合、`Invalid` ノードを生成してパースを部分的に中断：

- 必須トークンの欠落（`{`, `}`, `;`, `:` など）
- 識別子が期待される位置に別のトークンがある
- 予期しない入力終端

#### 3.2 弱いエラー（パース継続）

以下の場合、エラーを記録するがパースを継続：

- 関数呼び出しの引数リストの不正なカンマ
- 引数間のカンマ欠落

## テストケースの網羅性

現在のテストケース：

| テストケース | パス | カバーしているエラー |
|------------|------|-------------------|
| `unexpected_eof_001.ns` | `fails/syntax/` | 予期しない入力終端 |
| `unclosed_paren_001.ns` | `fails/syntax/` | 括弧が閉じられていない |

### 不足しているテストケース

- [ ] 関数呼び出しの予期しないカンマ
- [ ] 関数呼び出しのカンマ欠落
- [ ] 変数宣言のコロン欠落 (`let x` ではなく `let: x`)
- [ ] 関数宣言のコロン欠落
- [ ] セミコロン欠落
- [ ] 波括弧の欠落・不一致
- [ ] 識別子が期待される位置に別のトークンがある

## 改善提案

### より詳細なエラーメッセージ

現在のエラーメッセージは汎用的であり、具体的なコンテキストが不足している場合がある：

**現状**:
```
unexpected end of input
```

**改善案**:
```
unexpected end of input: expected closing parenthesis ')'
unexpected end of input: expected expression after operator '+'
```

### エラーリカバリの強化

より多くのエラーケースで「弱いエラー」方針を採用し、可能な限りパースを継続することで：

- 1回のコンパイルで複数のエラーを報告できる
- IDE での補完やシンタックスハイライトが改善される

### 位置情報の精度向上

現在、`SourceLocation` は開始位置と終了位置を記録しているが、以下の情報も有用：

- 行番号・カラム番号
- エラー箇所の前後のコンテキスト
- マルチバイト文字への対応
