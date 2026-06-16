# 汎用後置添字演算子（Postfix Subscript Operator）

## ステータス: 完了

## 概要

`(*next)[1] = val;` のような、任意の式に対する `[expr]` 後置添字演算子がパースエラーになる問題を修正する。

## 現象

```nospace
func: __main() {
  let: next; let: val;
  (*next)[1] = val;
}
```

上記コードで以下のエラーが発生する:

```
error: unexpected token: expected Token::Semicolon
  (internal: src/tree_parser/statement/mod.rs:118)
line:1 column:39
func:__main(){let:next;let:val;(*next)[1]=val;}
                                      ^
```

## 原因分析

### パーサーの制限

`[expr]`（添字アクセス）は `parse_to_expression_tree_factor` 内の `Token::Identifier` ケースでのみ処理されている（[expression/mod.rs](../../src/tree_parser/expression/mod.rs#L196-L203)）:

```rust
Some((Token::Identifier(id), _)) => {
    // ...
    if let Some((Token::BracketL, _)) = self.iter.peek() {
        self.iter.next();
        let index_expr = self.parse_to_expression_tree_root();
        match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
        return self.located(Expression::ArrayAccess(id, index_expr), start, end);
    }
    // ...
}
```

括弧式 `(expr)` の後に `[expr]` が来ても処理されないため、式パーサーは `(*next)` をパースした時点で式の解析を終了し、文パーサーが `;` を期待するところで `[` に遭遇してエラーになる。

### AST の制限

`Expression::ArrayAccess(String, Box<LocatedExpression>)` はベース式を `String`（識別子名）として保持しており、任意の式をベースとする添字アクセスを表現できない。

### パース時の挙動

`(*next)[1] = val;` をパースする過程:

1. 文パーサーが `*` を先頭に見て、式文として式パーサーを呼び出す
2. 式パーサーが `*` を単項演算子（Deref）としてスタックに積む
3. `(next)` を括弧式としてパース → `Variable("next")`
4. 単項演算をラップ → `Deref(Variable("next"))`
5. `[` はどの二項演算子にもマッチしないため、式解析が終了
6. 文パーサーが `;` を期待するが `[` を発見 → エラー

## 設計方針

### 方針: パース時脱糖（Desugar）

AST や下流モジュールへの変更を最小限にするため、パーサーで `(expr)[i]` を `*(expr + i)` に脱糖する。

仕様上 `arr[i]` は `*(&arr + i)` と同義であり、変数名に対する `[i]` は `&` で参照を取得してからのオフセットアクセスである。
一方、任意の式に対する `[i]` は、式の結果をアドレスとして扱い `*(expr + i)` となる。

この脱糖は既存の AST ノード（`Operation1(Deref, ...)`, `Operation2(Plus, ...)`）のみで表現可能であり、意味解析・コンパイラへの変更は不要。

### 既存 `ArrayAccess` との使い分け

| パターン | パース結果 | 備考 |
|----------|------------|------|
| `arr[i]` | `ArrayAccess("arr", i)` | 既存のまま維持。意味解析で配列サイズの検証が可能 |
| `(expr)[i]` | `Deref(Plus(expr, i))` | 新規。汎用的なポインタ算術 |
| `func()[i]` | `Deref(Plus(Function(...), i))` | 新規。関数戻り値をアドレスとして添字アクセス |
| `(*p)[i]` | `Deref(Plus(Deref(Variable("p")), i))` | 新規。デリファレンス結果への添字アクセス |

`ArrayAccess` を残す理由:
- 配列サイズの静的な把握（意味解析・最適化で利用）
- 既存の全テスト・下流モジュールとの互換性

## 修正対象

### Step 1: パーサー修正（`src/tree_parser/expression/mod.rs`）

`parse_to_expression_tree_factor` の末尾に後置 `[expr]` ループを追加する。

具体的には、`parse_to_expression_tree_factor` の構造を以下のように変更:

```rust
fn parse_to_expression_tree_factor(&mut self) -> Box<LocatedExpression> {
    let start = self.current_pos();
    let mut result = match self.iter.peek() {
        Some((Token::Number(_), _)) => { /* 既存 */ }
        Some((Token::Identifier(_), _)) => { /* 既存（ArrayAccess 含む） */ }
        Some((Token::ParenthesisL, _)) => { /* 既存 */ }
        Some((Token::Keyword(Keyword::If), _)) => { /* 既存 */ }
        Some((Token::BraceL, _)) => { /* 既存 */ }
        // ...
    };

    // 後置添字演算子: result[expr] → *(result + expr)
    loop {
        if let Some((Token::BracketL, _)) = self.iter.peek() {
            self.iter.next(); // '[' を消費
            let index_expr = self.parse_to_expression_tree_root();
            match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
            let end = self.current_pos();
            // (expr)[i] → *(expr + i) に脱糖
            let plus_expr = self.located(
                Expression::Operation2(Operator2::Plus, result, index_expr),
                start, end,
            );
            result = self.located(
                Expression::Operation1(Operator1::Deref, plus_expr),
                start, end,
            );
        } else {
            break;
        }
    }

    result
}
```

**注意点**:
- `Identifier` ケース内で `ArrayAccess` を返す既存ロジックは `return` で関数を抜けるため、後置ループの対象外となる。これは意図的な動作（`arr[i]` は `ArrayAccess` として保持）
- しかし `arr[1][2]` のような連鎖アクセスを可能にするには、`Identifier` ケースの `return` を除去して後置ループに統合するか、`ArrayAccess` 生成後にも後置ループが実行されるようにする必要がある
- `func()[i]` も同様に、関数呼び出しケースの `return` を除去する必要がある

**推奨する実装方針**: `Identifier` ケース内の `ArrayAccess` と関数呼び出しの早期 return を除去し、factor 内の match の結果を一旦 `result` に格納してから後置ループで処理する。これにより:
- `arr[1][2]` → `ArrayAccess("arr", 1)` → 後置 `[2]` → `Deref(Plus(ArrayAccess("arr", 1), 2))`
- `func()[1]` → `Function(...)` → 後置 `[1]` → `Deref(Plus(Function(...), 1))`

### Step 2: パーサーテスト追加

`src/tree_parser/expression/test.rs` にテストケースを追加:

- `(*p)[0]` → `Deref(Plus(Deref(Variable("p")), Factor(0)))`
- `(*p)[1]` → `Deref(Plus(Deref(Variable("p")), Factor(1)))`
- `(x + y)[2]` → `Deref(Plus(Plus(Variable("x"), Variable("y")), Factor(2)))`
- 既存 `arr[i]` テストが `ArrayAccess` のまま維持されることの確認

### Step 3: 結合テスト追加

`resources/tests/passes/` にテストケース追加:

- デリファレンス結果への添字アクセス: `(*p)[i]`
- 添字アクセスへの代入: `(*p)[1] = val`
- 複合代入: `(*p)[1] += val`

### Step 4（オプショナル）: 仕様ドキュメント更新

`docs/spec.md` の配列セクションに、任意の式に対する添字アクセスが可能であることを追記。

## 対象外

- `ArrayAccess` の AST 変更（互換性維持のため変更しない）
- 意味解析・コンパイラの変更（脱糖により不要）

## 回避策

本修正が適用されるまでは、`(*next)[1]` の代わりに `*(*next + 1)` と記述することで同等の動作が得られる。

```nospace
# NG: (*next)[1] = val;
# OK:
*(*next + 1) = val;
```

## 関連ファイル

- [src/tree_parser/expression/mod.rs](../../src/tree_parser/expression/mod.rs) - 式パーサー（主な修正対象）
- [src/tree_parser/expression/test.rs](../../src/tree_parser/expression/test.rs) - 式パーサーテスト
- [docs/spec.md](../../docs/spec.md) - 言語仕様（§ 配列）

## 実施内容

### 実施日: 2026-03-05

#### Step 1: パーサー修正（完了）

`src/tree_parser/expression/mod.rs` の `parse_to_expression_tree_factor` を修正:

- `match` ブロックの結果を `result` 変数に格納するよう変更
- `Identifier` ケースの関数呼び出し・配列アクセスの早期 `return` を除去し、`if/else if/else` チェーンに変更
- `match` ブロックの後に後置 `[expr]` ループを追加
  - `(expr)[i]` → `*(expr + i)` に脱糖（`Deref(Plus(expr, i))`）
- `arr[i]` は引き続き `ArrayAccess` として保持

#### Step 2: ユニットテスト追加（完了）

`src/tree_parser/expression/test.rs` に4件のテストを追加:

- `test_parse_postfix_subscript_deref_paren`: `(*p)[0]` のパース確認
- `test_parse_postfix_subscript_deref_paren_index_1`: `(*p)[1]` のパース確認
- `test_parse_postfix_subscript_expr_paren`: `(x + y)[2]` のパース確認
- `test_parse_array_access_still_produces_array_access`: `arr[0]` が `ArrayAccess` を返すことを確認

#### Step 3: 統合テスト追加（完了）

`resources/tests/passes/` に3件の統合テストを追加:

- `postfix_subscript_read.ns`: `(*p)[0]`, `(*p)[1]` による読み取り
- `postfix_subscript_write.ns`: `(*p)[0] = val`, `(*p)[1] = val` による書き込み
- `postfix_subscript_compound.ns`: `(*p)[0] += val`, `(*p)[1] += val` による複合代入

`resources/tests/test-manifest.yaml` に3件を登録。

#### テスト結果（完了）

- 全テスト通過（失敗 0 件）
- ユニットテスト: 374 passed
- 統合テスト: 1286 passed, 0 failed（wsc依存の 178 件は ignored）
