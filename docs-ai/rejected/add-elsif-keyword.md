# elsif キーワードの追加

## 概要

`else: if:` 構文を `elsif:` キーワードに置き換える。AST 構造（入れ子の `Expression::If`）は変更せず、トークンレベルでの糖衣構文として導入する。`else: if:` は非推奨とし、将来的に廃止を検討する。

## 背景

- 現状: `else: if: cond { ... }` と記述し、パーサーが `else:` + `if:` を peek で検出して再帰呼び出し
- 旧実装: `elsif:` キーワードが存在していた
- `elsif:` 導入の利点: パーサーの簡潔化（4トークン消費 → 2トークン）、意図の明示性向上

## 方針

**方式A: AST 不変**

- `Expression::If(cond, then_stmts, else_stmts)` の構造は変更なし
- `elsif:` を見たらパーサーが内部で再帰的に `If` ノードを生成（現行と同じ入れ子構造）
- 意味解析・インタプリタ・コンパイラの変更は不要

## 変更箇所

### Step 1: トークンパーサー

**ファイル**: `src/token_parser/mod.rs`

1. `Keyword` enum に `Elsif` を追加
2. `determine_keyword_or_identifier()` に `"elsif" => Token::Keyword(Keyword::Elsif)` を追加

### Step 2: ツリーパーサー（式）

**ファイル**: `src/tree_parser/expression/mod.rs`

`parse_to_expression_tree_if_impl()` の else 分岐処理を変更:

```
現在:
  } の後に Else を peek → Else 消費 → Colon 消費 → If を peek → 再帰

変更後:
  } の後に Elsif を peek → Elsif 消費 → Colon 消費 → cond を解析して If ノード生成（再帰）
  } の後に Else を peek → Else 消費 → Colon 消費 → If を peek → 再帰（後方互換のため残す or 削除）
```

具体的な変更:

```rust
// 現在のコード
let stats_false = match self.iter.peek() {
    Some((Token::Keyword(Keyword::Else), token_info)) => {
        let else_start = token_info.code_pointer;
        self.iter.next();
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        match self.iter.peek() {
            Some((Token::Keyword(Keyword::If), _)) => {
                // else: if: cond {}
                let if_expr = self.parse_to_expression_tree_if_impl();
                ...
            }
            _ => { ... }
        }
    }
    _ => { vec![] }
};

// 変更後
let stats_false = match self.iter.peek() {
    Some((Token::Keyword(Keyword::Elsif), token_info)) => {
        // elsif: cond {} — 再帰的に elsif チェーンも処理される
        let elsif_start = token_info.code_pointer;
        let if_expr = self.parse_to_expression_tree_elsif_impl();
        let end_pos = self.iter.peek()
            .map(|(_, info)| info.code_pointer)
            .unwrap_or(elsif_start);
        vec![LocatedStatement {
            statement: Statement::Expression(if_expr),
            location: SourceLocation::new(elsif_start, end_pos),
        }]
    }
    Some((Token::Keyword(Keyword::Else), token_info)) => {
        let else_start = token_info.code_pointer;
        self.iter.next();
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        // else: { ... } のみ（else: if: は廃止）
        match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
        let (stats, mut stats_err) = parse_to_statements(self.iter);
        if !stats_err.is_empty() {
            self.code_parse_error.append(&mut stats_err);
        }
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        stats
    }
    _ => { vec![] }
};
```

新しいメソッド `parse_to_expression_tree_elsif_impl()` を追加:

```rust
fn parse_to_expression_tree_elsif_impl(&mut self) -> Box<Expression> {
    let token = self.iter.next(); // elsif キーワードを消費
    assert!(matches!(token, Some((Token::Keyword(Keyword::Elsif), _))));

    if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
        return Box::new(Expression::Invalid(e));
    }
    let cond = self.parse_to_expression_tree_root();
    if let Err(e) = match_expect_token!(self, self.iter.next(), Token::BraceL) {
        return Box::new(Expression::Invalid(e));
    }

    let (stats_true, mut stats_err) = parse_to_statements(self.iter);
    if !stats_err.is_empty() {
        self.code_parse_error.append(&mut stats_err);
    }
    match_expect_token_unused!(self, self.iter.next(), Token::BraceR);

    let stats_false = match self.iter.peek() {
        Some((Token::Keyword(Keyword::Elsif), ...)) => { /* 再帰 */ }
        Some((Token::Keyword(Keyword::Else), ...)) => { /* else ブロック */ }
        _ => { vec![] }
    };
    Box::new(Expression::If(cond, stats_true, stats_false))
}
```

### Step 3: BNF 文法の更新

**ファイル**: `docs/grammar.bnf`

```bnf
# 変更前
if_stmt ::=
    | "if" ":" expr block ("else" ":" block)? ";"

# 変更後
if_stmt ::=
    | "if" ":" expr block elsif_chain? ";"

elsif_chain ::=
    | "elsif" ":" expr block elsif_chain?
    | "else" ":" block
```

### Step 4: 言語仕様の更新

**ファイル**: `docs/spec.md`

- if 文のセクションに `elsif:` の説明を追加
- `else: if:` を廃止（or 非推奨として残す）

### Step 5: シンタックスハイライトの更新

**ファイル**: `syntaxes/nospace.tmLanguage.json`

- `elsif` をキーワードとして追加

### Step 6: テストの更新・追加

- 既存の `else: if:` を使うテストを `elsif:` に書き換え
  - `resources/tests/passes/examples/e0-01-fibonacci.ns`
  - `resources/tests/passes/examples/e1-01-queue.ns`
  - `resources/tests/passes/legacy/legacy_009.ns`
- 新規テストケースの追加:
  - `elsif:` の基本動作テスト
  - `elsif:` チェーン（3分岐以上）のテスト
  - `elsif:` + `else:` の組み合わせテスト
  - `elsif:` を含む式の戻り値テスト

### Step 7: `else: if:` の扱い

**選択肢**:
- (a) 即座に廃止（パースエラーにする）
- (b) 非推奨として残す（パースは成功するが warning を出す）
- (c) 両方サポートし続ける

→ 後方互換性を考慮しなくてよいため **(a) 即座に廃止** が最も簡潔。

## 変更不要な箇所

以下のモジュールは AST 構造が変わらないため変更不要:

- `src/semantic_analyzer/` — `Expression::If` の処理はそのまま
- `src/interpreter/` — `ExecExpression::If` の処理はそのまま
- `src/compiler_ws/` — `ExecExpression::If` の処理はそのまま

## テスト戦略

1. 既存の全テストが通ること（`else: if:` を `elsif:` に書き換えた後）
2. `elsif:` 固有のテストが通ること
3. `else: if:` がエラーになること（廃止する場合）

## 作業見積もり

- Step 1 (トークンパーサー): 小規模（enum 1行 + match 1行）
- Step 2 (ツリーパーサー): 中規模（メソッド追加 + 既存メソッド修正）
- Step 3-5 (ドキュメント・ハイライト): 小規模
- Step 6 (テスト): 中規模

全体: 小〜中規模の変更
