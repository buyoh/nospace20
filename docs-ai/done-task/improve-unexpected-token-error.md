# unexpected token エラーメッセージに実際のトークン情報を追加

## 概要

構文解析時のエラーメッセージ `unexpected token: expected Token::Semicolon` が期待するトークンしか表示せず、実際に得られたトークンが不明。
エラーメッセージに「実際に得られたトークン」の情報を追加し、デバッグ効率を向上させる。

## 現状の問題

### エラーメッセージ例

```
at position 42: unexpected token: expected Token::Semicolon
```

- 期待トークンが Rust の `stringify!($pat)` で出力されるため `Token::Semicolon` のように内部表現がそのまま出る
- 実際に得られたトークンが表示されない

### 原因箇所

1. **`src/tree_parser/macros.rs`** — `match_expect_token!` マクロ
   - 3 つのバリアント全てで `Some((_, token_info))` とマッチし、実際のトークンを捨てている
   - エラーメッセージに `stringify!($pat)` のみ使用
2. **`src/tree_parser/expression/mod.rs:221`** — `"unexpected token"` (因子解析)
3. **`src/tree_parser/statement/mod.rs:1090`** — `"unexpected token"` (文解析)
4. **`src/tree_parser/mod.rs:34`** — `"unexpected token (unmatched closing brace or extra code)"`

### Token 型

`src/token_parser/mod.rs` に定義。`#[derive(Debug)]` のみで `Display` 未実装。

## 目標

- [x] `Token` 型に人間可読な表示メソッドを追加
- [x] `match_expect_token!` マクロで実際のトークンをエラーメッセージに含める
- [x] その他の "unexpected token" 箇所でも実際のトークンを含める
- [x] 期待トークンも人間可読な形式で表示する

## 設計

### Step 1: `Token::describe()` メソッドの追加

`src/token_parser/mod.rs` の `Token` に `describe()` メソッドを追加し、人間可読な短い文字列を返す。

```rust
impl Token {
    /// エラーメッセージ用の人間可読な説明を返す
    pub fn describe(&self) -> String {
        match self {
            Token::Number(n) => format!("number '{}'", n),
            Token::Identifier(s) => format!("identifier '{}'", s),
            Token::Keyword(k) => format!("keyword '{}'", k.as_str()),
            Token::StringLiteral(_) => "string literal".to_string(),
            Token::Plus => "'+'".to_string(),
            Token::Minus => "'-'".to_string(),
            Token::Asterisk => "'*'".to_string(),
            Token::Slash => "'/'".to_string(),
            Token::Percent => "'%'".to_string(),
            Token::Exclamation => "'!'".to_string(),
            Token::SingleEqual => "'='".to_string(),
            Token::DoubleEqual => "'=='".to_string(),
            Token::NotEqual => "'!='".to_string(),
            Token::Less => "'<'".to_string(),
            Token::Greater => "'>'".to_string(),
            Token::LessEqual => "'<='".to_string(),
            Token::GreaterEqual => "'>='".to_string(),
            Token::PlusEqual => "'+='".to_string(),
            Token::MinusEqual => "'-='".to_string(),
            Token::AsteriskEqual => "'*='".to_string(),
            Token::SlashEqual => "'/='".to_string(),
            Token::PercentEqual => "'%='".to_string(),
            Token::DoubleAmpersand => "'&&'".to_string(),
            Token::DoublePipe => "'||'".to_string(),
            Token::Ampersand => "'&'".to_string(),
            Token::ParenthesisL => "'('".to_string(),
            Token::ParenthesisR => "')'".to_string(),
            Token::BracketL => "'['".to_string(),
            Token::BracketR => "']'".to_string(),
            Token::BraceL => "'{'".to_string(),
            Token::BraceR => "'}'".to_string(),
            Token::Semicolon => "';'".to_string(),
            Token::Colon => "':'".to_string(),
            Token::Comma => "','".to_string(),
            Token::Invalid => "invalid token".to_string(),
        }
    }
}
```

`Keyword` にも `as_str()` メソッドを追加:

```rust
impl Keyword {
    pub fn as_str(&self) -> &'static str {
        match self {
            Keyword::Let => "let",
            Keyword::Func => "func",
            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::While => "while",
            Keyword::For => "for",
            Keyword::Repeat => "repeat",
            Keyword::Return => "return",
            Keyword::Break => "break",
            Keyword::Continue => "continue",
            Keyword::Static => "static",
            Keyword::Constexpr => "constexpr",
            Keyword::Alias => "alias",
            Keyword::Final => "final",
        }
    }
}
```

### Step 2: 期待トークンの表示名マッピング

`match_expect_token!` マクロの `stringify!($pat)` は `Token::Semicolon` のようなパターンを文字列化する。
これを人間可読にするため、ヘルパー関数 `describe_expected_token` を追加する。

```rust
/// stringify!() で生成される期待トークンパターン文字列を人間可読な形式に変換する
fn describe_expected_token(pat: &str) -> &str {
    match pat {
        "Token::Semicolon" => "';'",
        "Token::Colon" => "':'",
        "Token::Comma" => "','",
        "Token::ParenthesisL" => "'('",
        "Token::ParenthesisR" => "')'",
        "Token::BracketL" => "'['",
        "Token::BracketR" => "']'",
        "Token::BraceL" => "'{'",
        "Token::BraceR" => "'}'",
        "Token::SingleEqual" => "'='",
        "Token::Identifier(id)" | "Token::Identifier(_)" => "identifier",
        "Token::Number(_)" => "number",
        _ => pat, // フォールバック: そのまま表示
    }
}
```

**注意**: この関数は `tree_parser` モジュール内に配置する（マクロから呼び出すため）。

### Step 3: `match_expect_token!` マクロの修正

マクロの `Some((_, token_info))` を `Some((token, token_info))` に変更し、`token.describe()` をメッセージに含める。

```rust
macro_rules! match_expect_token {
    ($self: expr, $v: expr, $pat: pat) => {
        match $v {
            Some(($pat, _)) => Ok(()),
            Some((token, token_info)) => Err($self.add_parse_error(
                token_info,
                format!(
                    "unexpected token {}: expected {}",
                    token.describe(),
                    describe_expected_token(stringify!($pat))
                ),
            )),
            None => Err($self.add_end_error("unexpected end of input")),
        }
    };
    // ... 他のバリアントも同様
}
```

### Step 4: その他の "unexpected token" 箇所の更新

以下の箇所で、実際のトークンの `describe()` をメッセージに含める。

#### `src/tree_parser/expression/mod.rs:221`

```rust
// Before
let e = self.add_parse_error(token_info, "unexpected token");
// After (token をキャプチャする必要あり。直前の match arm を修正)
let e = self.add_parse_error(token_info, format!("unexpected token {}", token.describe()));
```

この箇所は `Some((_, token_info))` を `Some((token, token_info))` に変更する必要がある。

#### `src/tree_parser/statement/mod.rs:1090`

同様に `token` をキャプチャして `describe()` を使用。

#### `src/tree_parser/mod.rs:34`

```rust
// Before
"unexpected token (unmatched closing brace or extra code)"
// After
format!("unexpected token {} (unmatched closing brace or extra code)", token.describe())
```

この箇所は `(_, token_info)` を `(token, token_info)` に変更する必要がある。

## エラーメッセージの変化例

| Before | After |
|--------|-------|
| `unexpected token: expected Token::Semicolon` | `unexpected token '+': expected ';'` |
| `unexpected token: expected Token::ParenthesisR` | `unexpected token ';': expected ')'` |
| `unexpected token: expected Token::Identifier(id)` | `unexpected token number '42': expected identifier` |
| `unexpected token` | `unexpected token '+'` |
| `unexpected token (unmatched closing brace or extra code)` | `unexpected token '}' (unmatched closing brace or extra code)` |

## テストへの影響

影響するテストファイル:
- `resources/tests/fails/syntax/unexpected_factor_001.check.json` — `"contains": ["unexpected"]` ✓
- `resources/tests/fails/syntax/while_as_expression_001.check.json` — `"contains": ["unexpected"]` ✓
- `resources/tests/fails/syntax/void_while_assign_001.check.json` — `"contains": ["unexpected"]` ✓
- `resources/tests/fails/syntax/missing_colon_let_001.check.json` — `"contains": ["expected"]` ✓

ユニットテスト (`src/base/error/parse_error_tests.rs`) はハードコードされたエラー文字列を使っており、マクロ経由ではないため影響なし。

## 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/token_parser/mod.rs` | `Token::describe()` メソッド、`Keyword::as_str()` メソッド追加 |
| `src/tree_parser/macros.rs` | マクロ修正（実際のトークンをキャプチャ・表示） |
| `src/tree_parser/mod.rs` | `describe_expected_token()` 関数追加、余剰トークンエラー修正 |
| `src/tree_parser/expression/mod.rs` | "unexpected token" に `token.describe()` 追加 |
| `src/tree_parser/statement/mod.rs` | "unexpected token" に `token.describe()` 追加 |

## 作業規模

小規模。変更ファイル 5 つ、全て `tree_parser` / `token_parser` モジュール内に収まる。

## 実装結果 (2026-03-05)

### 実装完了

設計通りにすべての変更を適用した。全テスト合格（失敗なし）。

#### 追加したテストケース

- `resources/tests/fails/syntax/missing_semi_got_paren_001` — `')'` と `';'` 両方を含むことを確認
- `resources/tests/fails/syntax/unexpected_factor_semicolon_001` — `';'` を含むことを確認
- `resources/tests/fails/syntax/extra_code_plus_001` — `'+'` と `unmatched` を含むことを確認

#### 実装メモ

- `describe_expected_token()` は `pub(super)` として `tree_parser/mod.rs` に定義し、マクロから `super::describe_expected_token()` で呼び出す
- `code_parse_error!` マクロがフォーマット引数をサポートしないため、`format!()` で文字列を組み立ててから渡した

