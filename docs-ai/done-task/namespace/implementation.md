# 名前空間: 実装設計

名前空間機能の実装方針をモジュールごとに記載する。

## 設計の核心: コンパイル時マングリング

名前空間は**コンパイル時の名前修飾**として実装する。意味解析（semantic_analyzer）の段階で名前空間プレフィックスを識別子名に連結し、以降の処理（インタプリタ・コンパイラ）では通常の識別子として扱う。

マングリング規則:

```
namespace: A {
  let: x;       → 変数名 "A$x"
  func: f() {}  → 関数名 "A$f"
  namespace: B {
    let: y;     → 変数名 "A$B$y"
  }
}
```

区切り文字は `$`。識別子に `$` は使えないため、マングル名と非マングル名の区別は自明。
`.` ではなく `$` を採用する理由: `.` は将来の小数点や構造体フィールドアクセスと衝突するため。

## Step 1: Token Parser (`src/token_parser/`)

### 変更内容

1. **`Keyword` enum に `Namespace` を追加**

```rust
pub enum Keyword {
    // ... 既存 ...
    Namespace,
}
```

`as_keyword_token` でも `"namespace" => Some(Token::Keyword(Keyword::Namespace))` を追加。
`as_str` にも `Keyword::Namespace => "namespace"` を追加。

2. **`Token` enum に `Dollar` を追加**

```rust
pub enum Token {
    // ... 既存 ...
    Dollar,  // $
}
```

`parse_to_tokens_internal` でドル記号 `'$'` を `Token::Dollar` として認識する。
現在 `$` はどこにも使われていないため、衝突はない。

3. **`Token::describe` に `Dollar` の記述を追加**

```rust
Token::Dollar => "'$'",
```

### 影響範囲

- `mod.rs`: `Keyword` enum, `Token` enum, `as_keyword_token`, `as_str`, トークナイザ本体
- テスト: 新規のトークン化テストを追加

## Step 2: Tree Parser (`src/tree_parser/`)

### 変更内容

1. **`Statement` enum に `NamespaceDeclaration` を追加**

```rust
pub enum Statement {
    // ... 既存 ...
    /// 名前空間宣言: `namespace: Name { 文... }`
    NamespaceDeclaration(String, Vec<LocatedStatement>),
}
```

2. **`namespace:` のパース**

`parse_to_statements` のキーワードマッチに `Keyword::Namespace` を追加:

```
namespace: ident { stmt* }
```

- `namespace:` キーワードの後に識別子を期待
- `{` `}` でブロックを囲む
- **末尾セミコロンは不要**（`func:` と同じパターン）

3. **修飾識別子のパース（`$` アクセス）**

2つのアプローチを検討:

**方式A: トークナイザ段階で結合**

`parse_identifier_or_keyword` 内で、識別子の後に `$` + 識別子が続く場合、`$` 込みで一つの `Token::Identifier("A$x")` として返す。

利点:
- tree_parser, semantic_analyzer, interpreter, compiler_ws への修正が最小
- 修飾名は既存の `Identifier(String)` で自然に表現される

欠点:
- トークナイザが文脈依存的になる（ただし "$識別子" の規則は単純）
- 将来 `$` を別の目的に使う場合に再設計が必要

**方式B: tree_parser で結合**

`Token::Dollar` をそのまま出力し、tree_parser の式パース（expr_val）で `ident "$" ident "$" ...` を検出して `Expression::Variable("A$B$x")` に変換する。

利点:
- トークナイザが純粋なまま
- `$` の別の用途への拡張が容易

欠点:
- tree_parser の式パースが複雑化する（変数参照・関数呼び出しの両方に `$` 処理を追加）

**推奨: 方式B**

方式B を採用する。理由:
- トークナイザの責務はトークン分割に限定すべき
- tree_parser の expr_postfix レベルで `$ident` チェーンを展開すれば変更は局所的
- 関数呼び出し `A$f()` にも対応が必要であり、tree_parser での処理が自然

具体的には、`expr_val` での識別子パース後に `$` + 識別子のチェーンを貪欲に読み、結合した文字列を生成する:

```
expr_val:
  ident の後:
    while peek == Token::Dollar {
      consume Dollar
      expect ident
      name = name + "$" + next_ident
    }
    if peek == Token::ParenthesisL {
      関数呼び出し（修飾名）
    } else {
      Variable（修飾名）
    }
```

4. **制御構文の禁止チェック**

名前空間ブロックのパース時、内部のステートメントが制御構文（`while:`, `for:`, `repeat:`）でないことをチェックする。
ただし、`if:` は式として使われるため除外（式文として許可）。
→ 実際にはこのチェックは semantic_analyzer で行う方が適切。tree_parser はパースのみに専念する。

### 影響範囲

- `statement/mod.rs`: `Statement` enum, `parse_to_statements` のキーワードケース追加
- `expression/mod.rs`: `expr_val` の識別子パース拡張
- テスト: パーステスト追加

## Step 3: Semantic Analyzer (`src/semantic_analyzer/`)

**最も大きな変更が必要なモジュール。**

### 変更内容

1. **名前空間コンテキストの導入**

`AnalyzeContext` または `syntactic_analyze_impl` に名前空間プレフィックスを保持する:

```rust
struct NamespaceContext {
    /// 現在の名前空間プレフィックスのスタック（例: ["A", "B"] → "A$B$"）
    prefix_stack: Vec<String>,
}

impl NamespaceContext {
    fn current_prefix(&self) -> String {
        if self.prefix_stack.is_empty() {
            String::new()
        } else {
            self.prefix_stack.join("$") + "$"
        }
    }

    fn mangle(&self, name: &str) -> String {
        let prefix = self.current_prefix();
        if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}{}", prefix, name)
        }
    }
}
```

2. **`NamespaceDeclaration` の処理**

`syntactic_analyze_impl` のパス処理を拡張:

- **パス0 (constexpr / alias)**:
  `NamespaceDeclaration` 内の `constexpr` と `alias` を再帰的に収集し、マングル名で登録する。

- **パス1a (関数スキャン)**:
  `NamespaceDeclaration` 内の `FunctionDeclaration` をスキャンし、マングル名で登録する。

- **パス1b (変数スキャン)**:
  `NamespaceDeclaration` 内の `VariableDeclaration` をスキャンし、マングル名で登録する。

- **パス2 (文の変換)**:
  `NamespaceDeclaration` を処理する際:
  1. 名前空間プレフィックスをスタックにプッシュ
  2. 内部の文を再帰的に処理
  3. 名前空間プレフィックスをスタックからポップ
  4. `NamespaceDeclaration` 自体は出力に含めない（フラット化される）

3. **名前解決の修正**

`ScopeResolver` の `resolve_variable` / `resolve_function` / `resolve_constexpr` / `resolve_alias_chain` に名前空間コンテキストを渡す。

名前解決の追加ルール:
- 変数名 `x` が来たとき、まず `{prefix}$x` で探索
- 見つからなければ `x` で通常の探索
- `$` 付き名前 `A$x` はそのまま探索

具体的には、名前解決時に名前空間プレフィックスを考慮するラッパーを用意する:

```rust
fn resolve_with_namespace(
    resolver: &ScopeResolver,
    ns_ctx: &NamespaceContext,
    name: &str,
) -> Option<IdentifierRef> {
    // 名前が `$` を含む場合（修飾名）→ そのまま解決
    if name.contains('$') {
        // 絶対名として探索
        // 現在の名前空間が "A$B" で、name が "C$x" の場合
        // まず "A$B$C$x" を試し、なければ "A$C$x"、"C$x" と外側に向かって探索
        // → 相対解決
        ...
    }
    // 名前に `$` が含まれない場合
    // 1. プレフィックス付きで探索
    let mangled = ns_ctx.mangle(name);
    if let Some(r) = resolver.resolve_variable(&mangled) {
        return Some(r);
    }
    // 2. プレフィックスなしで探索
    resolver.resolve_variable(name)
}
```

4. **名前空間名の重複チェック**

同一スコープ内で同じ名前空間名が複数回出現した場合をエラーにする。
パス0またはパス1で名前空間名のセットを管理する。

5. **名前空間名と変数名・関数名の衝突チェック**

名前空間名は変数名や関数名として使用できない（修飾識別子の曖昧性を避けるため）。

### 修飾名の相対解決

名前空間 `A$B` の内部で `C$x` を参照した場合の解決手順:

1. `A$B$C$x` を探索（現在の名前空間のサブ名前空間）
2. `A$C$x` を探索（親の名前空間のサブ名前空間）
3. `C$x` を探索（グローバルの名前空間のサブ名前空間）
4. 見つからなければエラー

これは、C++ の名前空間解決と同様のセマンティクス。

### 影響範囲

- `mod.rs`: `syntactic_analyze_impl` の全パスの拡張
- `context.rs`: `AnalyzeContext` への名前空間情報追加
- `scope.rs`: `ScopeResolver` の名前解決拡張
- `alias.rs`, `constexpr.rs`: マングル名での収集対応
- `expression.rs`: 変数参照・関数呼び出しの名前解決に名前空間プレフィックスを渡す
- `statement.rs`: `NamespaceDeclaration` の処理、マングル名での変数・関数登録

## Step 4: Interpreter (`src/interpreter/`)

### 変更なし

意味解析の段階でマングリングが完了しているため、インタプリタへの変更は不要。
マングル名（例: `A$x`）は通常の変数スロットインデックスに解決されており、インタプリタはインデックスベースでアクセスする。

## Step 5: Compiler WS (`src/compiler_ws/`)

### 変更なし（または最小限）

意味解析でマングリングが完了しているため、コンパイラへの変更は本質的に不要。
SymbolTable のラベル生成にマングル名が使用されるが、`$` を含む文字列がラベルに使われても Whitespace のラベルはバイナリ形式であり問題ない。

念のため確認事項:
- `label.rs`: ラベル名に `$` を含む識別子が渡されても正常に動作するか確認
- `memory.rs`: ヒープアドレスの割り当てにマングル名が影響しないか確認

## Step 6: Optimizer (`src/optimizer/`)

### 変更なし

最適化パスは ExecStatement / ExecExpression レベルで動作し、識別子名を直接参照しない（インデックスベース）。

## Step 7: WASM API (`src/wasm_api/`)

### 変更なし

公開 API はソースコード文字列を受け取り、結果を返すものであり、内部の名前解決に影響しない。

## Step 8: Grammar / Syntax (`syntaxes/`)

### 変更内容

1. `grammar.bnf` に名前空間の規則を追加
2. `nospace.tmLanguage.json` に `namespace` キーワードのハイライト定義を追加

## 実装順序

1. **Step 1: Token Parser** — `Namespace` キーワード、`Dollar` トークン追加
2. **Step 2: Tree Parser** — `NamespaceDeclaration` のパース、修飾識別子のパース
3. **Step 3: Semantic Analyzer** — マングリング、名前解決拡張（最大の作業量）
4. **Step 8: Grammar / Syntax** — BNF・シンタックスハイライト更新
5. テスト追加・動作確認

Step 4-7 は変更不要のため省略可能。

## テンプレート関数・alias との相互作用

### alias のターゲットに修飾名を使用可能

```
namespace: Math {
  func: add(a, b) { return: a + b; }
}
alias: add(Math$add);
add(1, 2);  # Math$add(1, 2) と同等 #
```

### テンプレートのインスタンス化と名前空間

名前空間内でテンプレートをインスタンス化した場合、インスタンス名にプレフィックスが付加される。

```
func: tmpl(x), alias: constexpr: n { return: x + n; }
namespace: Funcs {
  alias: add5(tmpl, 5);   # マングル名: Funcs$add5 #
}
Funcs$add5(10);  # 15 #
```

## 将来の拡張への影響

### include との関係

将来 `include:` 文が実装された場合、インクルードされたファイルの内容が名前空間内に配置されるユースケースが考えられる:

```
namespace: MyLib {
  include: "mylib.ns";
}
```

名前空間のマングリング機構はこのユースケースに自然に対応できる。

### 型システムとの関係

型システムが導入された場合、型名にも名前空間プレフィックスを付加することで対応可能。

### セルフコンパイラ (nospace-core) との関係

名前空間は nospace-core の縮小仕様には含めない予定（nospace-core はミニマルな仕様を目指す）。
