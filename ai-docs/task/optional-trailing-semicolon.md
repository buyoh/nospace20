# ブロック末尾セミコロン省略の設計

## 概要

「ブロック `{ ... }` またはプログラム末尾の最後のステートメントでは `;` を省略しても良い」という仕様を新たに追加する場合の影響範囲と修正方針を整理する。

## 仕様の定義

### ルール

- ブロック `{ ... }` 内で、閉じ中括弧 `}` の直前にある最後のステートメントは `;` を省略できる
- プログラムのトップレベルで、末尾のステートメントは `;` を省略できる
- `;` を書いても引き続き有効（後方互換性を維持）

### 適用対象

以下のすべてのステートメント種別に適用される:

| ステートメント種別 | 現在 | 変更後（末尾のみ） |
|---|---|---|
| 式文 | `expr;` | `expr` でも可 |
| 変数宣言 | `let: x;` | `let: x` でも可 |
| static宣言 | `static: x;` | `static: x` でも可 |
| return文 | `return: expr;` | `return: expr` でも可 |
| break文 | `break;` | `break` でも可 |
| continue文 | `continue;` | `continue` でも可 |

> `func:` 宣言はもともと `;` 不要（ブロック `{ }` で終端）。

### 適用コンテキスト

`;` の省略が許可されるのは、次のトークンが以下のいずれかの場合:

1. `}` — ブロック・関数本体の終端
2. EOF — プログラムの終端

## 影響範囲の分析

### 修正が必要なモジュール

| モジュール | ファイル | 修正内容 | 規模 |
|---|---|---|---|
| **tree_parser (statement)** | `src/tree_parser/statement/mod.rs` | セミコロン消費ロジックの変更 | **中** |
| **tree_parser (macros)** | `src/tree_parser/macros.rs` | 新マクロ追加（任意） | **小** |

### 修正不要なモジュール

| モジュール | 理由 |
|---|---|
| token_parser | `;` のトークン化に変更なし |
| tree_parser (expression) | 式パーサはセミコロンを扱わない |
| semantic_analyzer | AST レベルで動作、トークンに依存しない |
| interpreter | AST/Scope レベルで動作 |
| compiler_ws | AST/Scope レベルで動作 |
| wasm_api | パイプラインを呼び出すだけ |

### 仕様・ドキュメント修正

| ファイル | 修正内容 |
|---|---|
| `docs/spec.md` | セミコロン省略ルールの追記。while/if の「末尾に `;` が必要」の注記を修正 |
| `docs/grammar.bnf` | BNF の `stmt`, `global_stmt`, `block` 規則を修正 |
| `docs/tutorial.md` | 必要に応じて更新 |

### テスト

| テスト種別 | ファイル | 修正内容 |
|---|---|---|
| Unit テスト | `src/tree_parser/statement/test.rs` | セミコロン省略パターンのテスト追加 |
| Large テスト | `resources/tests/passes/` | 新規 `.ns`+`.check.json` ファイル追加 |
| 既存テスト | 全既存テスト186件 | **変更不要**（後方互換） |

## 具体的な修正方針

### 1. `src/tree_parser/statement/mod.rs` の変更

#### 1-1. ヘルパーメソッドの追加

ブロック末尾・EOF であればセミコロンを消費せずスキップするヘルパーを追加:

```rust
/// セミコロンを消費する。ただし、次のトークンが `}` または EOF の場合はスキップ（省略許可）。
fn consume_semicolon_or_end(&mut self) {
    match self.iter.peek() {
        Some((Token::Semicolon, _)) => {
            self.iter.next(); // セミコロンを消費
        }
        Some((Token::BraceR, _)) | None => {
            // ブロック末尾 or EOF — セミコロン省略を許可
        }
        Some((_, token_info)) => {
            // セミコロンでもブロック末尾でもない — エラー
            self.add_parse_error(token_info, "expected ';'");
        }
    }
}
```

#### 1-2. 既存セミコロン消費箇所の置き換え

以下の5箇所で `match_expect_token_unused!(self, self.iter.next(), Token::Semicolon)` を `self.consume_semicolon_or_end()` に置き換え:

| 行 (概算) | コンテキスト |
|---|---|
| L511 | `parse_variable_declarations` 末尾 |
| L604 | `parse_to_statements_return` 末尾 |
| L643 | `parse_to_statements` 内 break |
| L657 | `parse_to_statements` 内 continue |
| L676 | `parse_to_statements` 内 式文 |

#### 1-3. エラーリカバリの「セミコロンまでスキップ」パターン

`parse_variable_declarations` 内のエラーリカバリで `matches!(token, Token::Semicolon)` でスキップしている箇所（約6箇所）は、`Token::BraceR` でもスキップを停止するよう修正が必要:

```rust
// 変更前
while let Some((token, _)) = self.iter.peek() {
    if matches!(token, Token::Semicolon) {
        break;
    }
    self.iter.next();
}

// 変更後
while let Some((token, _)) = self.iter.peek() {
    if matches!(token, Token::Semicolon | Token::BraceR) {
        break;
    }
    self.iter.next();
}
```

ただし、これらのリカバリ箇所でセミコロンを消費（`self.iter.next()`）している箇所も、`consume_semicolon_or_end` や peek による条件分岐に変更する必要がある。

### 2. `docs/grammar.bnf` の変更

```bnf
# 変更前
block ::= "{" stmt* "}"

# 変更後
block ::= "{" (stmt ";")* stmt? "}"   # 最後の stmt は ";" が省略可能
```

`stmt` 定義からも `;` を分離するか、`block` のルールで表現するか、設計上の選択がある。

### 3. `docs/spec.md` の変更

- セミコロン省略ルールの新セクション追加
- while/if の説明にある「while は式であるため、末尾に `;` が必要」の注記を補足修正

### 4. テスト追加

#### Unit テスト (`src/tree_parser/statement/test.rs`)

- 各ステートメント種別で末尾 `;` 省略パターン（ブロック末尾、トップレベル末尾）
- `;` を省略した後に `}` が来るケース
- 中間のステートメントで `;` を省略した場合のエラーケース

#### Large テスト (`resources/tests/`)

- `resources/tests/passes/` に省略構文を使ったテストケース（3〜5ファイル程度）
- `resources/tests/fails/syntax/` に不正な省略（中間のステートメントで省略）のエラーテスト

## 修正規模の見積もり

| カテゴリ | 規模 |
|---|---|
| **パーサ本体** (`statement/mod.rs`) | ~30行変更（ヘルパー追加 + 5箇所の置換 + エラーリカバリ6箇所） |
| **仕様ドキュメント** (`docs/spec.md`, `grammar.bnf`) | ~15行追加・変更 |
| **Unit テスト** | ~80行追加 |
| **Large テスト** | ~5ファイル追加 |
| **合計** | **小〜中規模**（全体で100〜150行程度の変更・追加） |

## リスクと注意点

### 曖昧性

- セミコロン省略は**ブロック末尾 / EOF** のみに限定されるため、文法の曖昧性は発生しない
- 式パーサは `;` を認識しないため、式の終端判定に影響しない

### ブロックスコープ式との相互作用

- `x = { let: a(3); a }` のように最後の式でセミコロンを省略する記法が自然になる
- 既存の `x = { let: a(3); a; };` も引き続き有効

### エラーリカバリの品質

- セミコロン省略により「セミコロンまでスキップ」のエラーリカバリが `}` も考慮する必要がある
- エラーメッセージの品質に影響する可能性がある（例: 中間のステートメントでセミコロンを書き忘れた場合のエラー位置がずれる可能性）

### 後方互換性

- 既存コードはすべて引き続き有効（`;` を書くのは常に許可）
- 186件の既存テストケースに影響なし
