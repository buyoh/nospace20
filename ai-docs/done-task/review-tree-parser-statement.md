# コードレビュー: tree_parser/statement/mod.rs

## 対象ファイル

- `src/tree_parser/statement/mod.rs` (684行)
- 関連: `src/tree_parser/macros.rs`, `src/tree_parser/statement/test.rs`

## レビュー日

2026-02-25

## 概要

`StatementBuilder` による文の構文解析ロジックのレビュー。変数宣言（通常変数・配列・文字列初期化）、関数宣言、制御文（break/continue/return）のパースを担当する。

---

## 発見事項

### Refactor-1: `parse_variable_declarations` が巨大 (~350行)

**深刻度**: 中 (保守性)

1つのメソッドに以下のすべてが詰め込まれている:

1. 識別子の取得
2. 配列サイズ `[N]` / `[]` のパース
3. 通常変数の初期化 `(expr)`
4. 配列の文字列初期化 `("Hello")`
5. 配列のリスト初期化 `([1, 2, 3])`
6. 初期化なしの場合のバリデーション
7. 複数変数のカンマ区切りループ

**対処案**: 以下のようにサブメソッドに分割する:
- `parse_array_size()` → `(bracket_specified, array_size)`
- `parse_array_string_init()` → 文字列初期化の処理
- `parse_array_list_init()` → リスト初期化の処理
- `parse_variable_init()` → 通常変数の初期化

---

### Refactor-2: エラーリカバリパターンの重複 (7箇所以上)

**深刻度**: 中 (保守性・DRY)

以下のパターンが少なくとも7回重複:

```rust
while let Some((token, _)) = self.iter.peek() {
    if matches!(token, Token::Semicolon) {
        break;
    }
    self.iter.next();
}
self.iter.next(); // セミコロンを消費
return results;
```

**対処案**: ヘルパーメソッドを導入:

```rust
fn skip_to_semicolon(&mut self) {
    while let Some((token, _)) = self.iter.peek() {
        if matches!(token, Token::Semicolon) {
            break;
        }
        self.iter.next();
    }
    self.iter.next(); // セミコロンを消費
}
```

※ `optional-trailing-semicolon.md` タスクでも `Token::BraceR` 追加の必要性が指摘されており、ヘルパー化すれば一括対応可能。

---

### Refactor-3: `end_pos` 計算パターンの重複 (5箇所以上)

**深刻度**: 低 (保守性)

以下のパターンが複数回出現:

```rust
let end_pos = self
    .iter
    .peek()
    .map(|(_, info)| info.code_pointer)
    .unwrap_or(start_pos);
```

**対処案**: ヘルパーメソッドを導入:

```rust
fn current_pos_or(&self, default: usize) -> usize {
    self.iter.peek()
        .map(|(_, info)| info.code_pointer)
        .unwrap_or(default)
}
```

---

### Refactor-4: `parse_to_statements_let` / `parse_to_statements_static` の重複

**深刻度**: 低 (DRY)

2つのメソッドはほぼ同一で、キーワードチェックと `is_static` フラグのみが異なる:

```rust
fn parse_to_statements_let(&mut self, start_pos: usize) -> Vec<LocatedStatement> {
    match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Let)); // ← ここだけ違う
    self.parse_variable_declarations(start_pos, false)                         // ← false/true
}
```

**対処案**: 1つのメソッドに統合し、引数でキーワードと `is_static` を渡す。

---

### Quality-1: エラー位置が不正確

**深刻度**: 低 (UX)

配列サイズや初期化関連のエラーで `start_pos`（`let`/`static` キーワードの位置）を使用している:

```rust
let err_idx = self.add_parse_error(
    &TokenInfo { code_pointer: start_pos },  // ← let キーワードの位置
    "array size must be positive",
);
```

実際にエラーが発生したトークン（サイズ値 `0` など）の位置を使うべき。

**対処案**: `self.iter.next()` で取得したトークンの `token_info` を直接使用する。

---

### Quality-2: 不要な `return` キーワード (Rust イディオム)

**深刻度**: 低 (スタイル)

以下の箇所で明示的な `return` が使われているが、Rust では最後の式が暗黙的に返される:

- `parse_to_statements_block`: `return ss;`
- `parse_to_statements_func`: `return LocatedStatement { ... };`
- `parse_to_statements_return`: `return LocatedStatement { ... };`
- `parse_to_statements`: `return statements;`

---

### Quality-3: `if let Err(_) = ... { panic!("internal error") }` パターン

**深刻度**: 低 (スタイル)

```rust
if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Let)) {
    panic!("internal error");
}
```

内部一貫性チェックとして使われているが、`debug_assert!` や `.unwrap()` の方が意図が明確。
あるいは呼び出し元で既に `Token::Keyword(Keyword::Let)` を確認しているため、このチェック自体が冗長。

---

### Quality-4: `match_expect_token_unused!` による暗黙的なエラー継続

**深刻度**: 低 (堅牢性)

`match_expect_token_unused!` はエラーを `code_parse_error` に記録するが、制御フローは変更しない:

```rust
match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
// ← ']' が無くてもそのまま続行
```

閉じ括弧 `]` / `)` が欠落した場合、後続のパースがずれてカスケードエラーを引き起こす可能性がある。
重要な区切りトークンについてはエラー時に早期リターンを検討すべき。

---

### Quality-5: 関数引数パースのエッジケース

**深刻度**: 低 (堅牢性)

`parse_to_statements_func` の引数パース部分:
- `State::Comma` の後に `)` が来た場合（`func: f(x,)`）のエラーメッセージが "unexpected ','" でカンマの位置ではなく `)` の位置を指す
- 予期しないトークンで `break` した場合、そのトークンは消費されずに残る

---

## 改善優先度まとめ

| # | 種別 | 項目 | 優先度 | 工数 |
|---|------|------|--------|------|
| Refactor-1 | リファクタ | `parse_variable_declarations` 分割 | 中 | 大 |
| Refactor-2 | リファクタ | エラーリカバリの共通化 | 中 | 中 |
| Refactor-3 | リファクタ | `end_pos` 計算の共通化 | 低 | 小 |
| Refactor-4 | リファクタ | let/static メソッド統合 | 低 | 小 |
| Quality-1 | 品質 | エラー位置の精度 | 低 | 中 |
| Quality-2 | スタイル | 不要な `return` | 低 | 小 |
| Quality-3 | スタイル | internal panic パターン | 低 | 小 |
| Quality-4 | 品質 | `match_expect_token_unused!` 制御フロー | 低 | 中 |
| Quality-5 | 品質 | 関数引数パースのエッジケース | 低 | 小 |

## 関連タスク

- [optional-trailing-semicolon.md](optional-trailing-semicolon.md) — Refactor-2 のエラーリカバリ共通化と連動
- [error-test-coverage/](error-test-coverage/) — Quality-4, Quality-5 のテスト追加に関連

## 備考

- テストファイル `test.rs` (594行) は正常系・エラー系の基本的なケースをカバーしている

## 実装結果 (2026-02-25)

以下をすべて実施した。全テスト pass (0 failed)。

| # | 実施内容 |
|---|---------|
| Refactor-1 | `parse_variable_declarations` を `parse_array_size` / `parse_array_string_init` / `parse_array_list_init` / `parse_variable_init` に分割 |
| Refactor-2 | `skip_to_semicolon()` ヘルパーメソッドを追加し、7箇所の重複パターンを置き換え |
| Refactor-3 | `current_pos_or()` ヘルパーメソッドを追加し、5箇所の重複パターンを置き換え |
| Refactor-4 | `parse_to_statements_let` / `parse_to_statements_static` を `parse_to_statements_variable(is_static)` に統合 |
| Quality-1 | 配列サイズエラーの位置を `start_pos` からサイズ値のトークン位置に修正 |
| Quality-2 | `parse_to_statements_block` / `parse_to_statements_func` / `parse_to_statements_return` / `parse_to_statements` の不要な `return` を削除 |
| Quality-3 | `if let Err(_) = ... { panic! }` パターンを削除（呼び出し元でキーワード確認済みのため `iter.next()` のみに簡略化） |
| Quality-5 | trailing comma `func: f(x,)` のエラーメッセージを "unexpected ','" から "trailing ','" に改善 |
| テスト追加 | `test_parse_func_trailing_comma_error`, `test_parse_func_leading_comma_error`, `test_parse_array_zero_size_has_error`, `test_parse_static_variable` を追加 |

未実施: Quality-4 (`match_expect_token_unused!` の制御フロー改善) — 影響範囲が大きいため別タスクとして管理推奨。
