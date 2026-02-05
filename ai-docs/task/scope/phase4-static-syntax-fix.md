# Phase 4 構文修正: `static: x;`

## 概要

spec.md の仕様に基づき、static 変数の構文を `static: x;` に修正する。

## 現状

### 現在の実装（誤り）
```nospace
let: static x;   # 現在の実装 #
```

### 仕様（spec.md）
```nospace
static: x;       # 正しい構文 #
```

## 修正内容

### 1. BNF

```bnf
# 変更前
let ::= "let" ":" identifier ("," identifier)* ";"

# 変更後
let    ::= "let" ":" identifier ("," identifier)* ";"
static ::= "static" ":" identifier ("," identifier)* ";"
```

### 2. 変更ファイル

#### 2.1 tree_parser/statement/mod.rs

**変更前**:
```rust
fn parse_to_statements_let(&mut self, start_pos: usize, is_static: bool) -> LocatedStatement {
    // ...
    // Phase 4: static修飾子のチェック (let: static x; 構文)
    let is_static = if let Some((Token::Keyword(Keyword::Static), _)) = self.iter.peek() {
        self.iter.next(); // consume 'static'
        true
    } else {
        is_static
    };
    // ...
}
```

**変更後**:
```rust
// parse_to_statements_let から static チェックを削除

// 新規追加: parse_to_statements_static
fn parse_to_statements_static(&mut self, start_pos: usize) -> LocatedStatement {
    if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Static)) {
        panic!("internal error");
    }
    match_expect_token_unused!(self, self.iter.next(), Token::Colon);
    let id = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
        Ok(x) => x,
        Err(e) => {
            return LocatedStatement {
                statement: Statement::Invalid(e),
                location: SourceLocation::from_single(start_pos),
            };
        }
    };
    let end_pos = self
        .iter
        .peek()
        .map(|(_, info)| info.code_pointer)
        .unwrap_or(start_pos);
    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
    return LocatedStatement {
        statement: Statement::VariableDeclaration(id.clone(), Box::new(Expression::Factor(0)), true),
        location: SourceLocation::new(start_pos, end_pos),
    };
}
```

#### 2.2 parse_to_statements の変更

```rust
match &token.0 {
    Token::Keyword(Keyword::Let) => {
        statements.push(self.parse_to_statements_let(start_pos));
        continue;
    }
    Token::Keyword(Keyword::Static) => {
        statements.push(self.parse_to_statements_static(start_pos));
        continue;
    }
    // ...
}
```

### 3. テストケースの修正

全てのテストファイルで `let: static x;` を `static: x;` に修正:
- `scope_static_001.ns`
- `scope_static_nested_001.ns`
- `scope_static_mixed_001.ns`
- `scope_static_multi_decl_001.ns`
- `scope_static_counter_factory_001.ns`

### 4. 実装手順

1. tree_parser: `parse_to_statements_let` から static チェックを削除
2. tree_parser: `parse_to_statements_static` を新規作成
3. tree_parser: `parse_to_statements` に Static ケースを追加
4. テストファイルの構文を修正
5. 全テストを実行して確認

## 仕様上のポイント

spec.md より:
- static 変数は、グローバルスコープの変数と同じタイミングで初期化される
- static 変数が定義された関数が呼び出されても初期化されない
- これは C 言語の static 変数と同様の動作

## 現時点での制限

- ネスト関数（Phase 5）が未実装のため、static 変数の本来の用途（親関数スコープへのアクセス）はテストできない
- Phase 5 実装後にテストを有効化する
