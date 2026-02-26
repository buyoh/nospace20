# Phase 2: 式レベルの位置情報 - 全体設計

## 背景

Phase 1 では `LocatedExecStatement` を導入し、コンパイルエラーに文の開始位置を表示できるようにした。
しかし `compiler_ws` のエラーには式レベルのエラー（代入左辺の不正、`&` の不正使用、`--std-ext alloc` 未有効時の `__alloc`使用等）があり、
現状は含む文の先頭位置しか報告できない。

Phase 2 では `Expression` / `ExecExpression` に `SourceLocation` を付与し、式の精度でエラー位置を報告する。

## 設計方針

### LocatedExpression ラッパーパターン

`LocatedStatement` / `LocatedExecStatement` と同じパターンで `LocatedExpression` / `LocatedExecExpression` ラッパー structを導入する。

```rust
// tree_parser 側
pub struct LocatedExpression {
    pub expression: Expression,
    pub location: SourceLocation,
}

// semantic_analyzer 側
pub struct LocatedExecExpression {
    pub expression: ExecExpression,
    pub location: SourceLocation,
}
```

### 変更の影響範囲

`Expression` を含む全ての箇所で `Box<Expression>` → `Box<LocatedExpression>` に変更する必要がある。
これは以下に影響する:

| カテゴリ | 対象 | 変更量 |
|---------|------|-------|
| 型定義 | `Expression` enum の各バリアント | 大 |
| 型定義 | `Statement` enum の各バリアント | 中 |
| 型定義 | `ExecExpression` enum の各バリアント | 大 |
| 型定義 | `ExecStatement` enum の各バリアント | 小 |
| パーサー | `tree_parser/expression/mod.rs` | 大 |
| パーサー | `tree_parser/statement/mod.rs` | 小 |
| 意味解析 | `semantic_analyzer/mod.rs` | 大 |
| コンパイラ | `compiler_ws/expression.rs` | 中 |
| コンパイラ | `compiler_ws/statement.rs` | 小 |
| インタプリタ | `interpreter/exec.rs` | 中 |
| テスト | `semantic_analyzer/tests.rs` 等 | 小 |

### 位置情報のキャプチャ方法

`ExpressionBuilder` の各パースメソッドで、**式の最初のトークン**の `code_pointer` を `start` とし、**式の最後のトークンの次のトークン**（もしくはパース終了位置）を `end` とする。

実装上は各パースメソッドが `Box<LocatedExpression>` を返すよう変更し、パースの開始時に `start_pos` をキャプチャする:

```rust
fn parse_to_expression_tree_factor(&mut self) -> Box<LocatedExpression> {
    let start = self.current_pos();
    let expr = match self.iter.peek() { /* ... */ };
    let end = self.current_pos();
    Box::new(LocatedExpression {
        expression: expr,
        location: SourceLocation::new(start, end),
    })
}
```

二項演算のように左右のノードを組み合わせるケースでは:

```rust
fn parse_to_expression_tree_plus(&mut self) -> Box<LocatedExpression> {
    let left = self.parse_to_expression_tree_mul();
    let start = left.location.start;  // 左辺の開始位置を継承
    // ...
    let right = self.parse_to_expression_tree_mul();
    let end = right.location.end;     // 右辺の終了位置
    Box::new(LocatedExpression {
        expression: Expression::Operation2(op, left, right),
        location: SourceLocation::new(start, end),
    })
}
```

### `current_pos()` ヘルパー

`ExpressionBuilder` に現在のピーク位置を返すヘルパーを追加:

```rust
impl ExpressionBuilder {
    fn current_pos(&mut self) -> usize {
        self.iter
            .peek()
            .map(|(_, info)| info.code_pointer)
            .unwrap_or(0)
    }
}
```

`StatementBuilder` には既に同様の `current_pos_or()` メソッドが存在する。

## 作業ステップ

### Step 1: tree_parser への LocatedExpression 導入

**対象ファイル**:
- `src/tree_parser/expression/mod.rs` - `LocatedExpression` 定義 + 全パースメソッド変更
- `src/tree_parser/statement/mod.rs` - `Statement` バリアントの型変更
- `src/tree_parser/mod.rs` - 再エクスポート

**詳細**: [step1-tree-parser.md](step1-tree-parser.md)

### Step 2: semantic_analyzer への LocatedExecExpression 導入

**対象ファイル**:
- `src/semantic_analyzer/types.rs` - `LocatedExecExpression` 定義 + `ExecExpression`/`ExecStatement` 型変更
- `src/semantic_analyzer/mod.rs` - `convert_to_exec_expression_with_resolver` の引数・戻り値型変更、位置伝搬

**詳細**: [step2-semantic-analyzer.md](step2-semantic-analyzer.md)

### Step 3: compiler_ws / interpreter の対応

**対象ファイル**:
- `src/compiler_ws/expression.rs` - `generate_expression` の引数変更、式ごとの `set_location`
- `src/compiler_ws/statement.rs` - `ExecStatement` 内の型変更対応
- `src/interpreter/exec.rs` - `interpret_expression` の引数変更
- `src/compiler_ws/context.rs` - `set_location` の使い方更新（任意）

**詳細**: [step3-compiler-interpreter.md](step3-compiler-interpreter.md)

### Step 4: テスト修正・検証

- 既存 Unit テスト修正
- 既存 Large テスト全パス確認
- 式レベルのエラー位置が正しく報告されることの確認

## 横断的な注意点

### `Expression::Invalid` の位置

`Expression::Invalid` は既にパースエラー時に生成される。`LocatedExpression` ラッパーにすることで、不正な式のソース位置も保持できる。

### `LocatedExecExpression` のメモリオーバーヘッド

`SourceLocation` は `(usize, usize)` = 16 bytes。
全ての `ExecExpression` ノードにアタッチされるため、メモリ使用量は増加する。
nospace プログラムの規模（通常数百〜数千ノード）では問題にならない。

### `ExecExpression` を Box で包む既存パターンとの整合

現在 `ExecExpression` は `Box<ExecExpression>` で再帰的に包まれている。
Phase 2 ではこれを `Box<LocatedExecExpression>` に変更する。
`LocatedExecExpression { expression: ExecExpression, location: SourceLocation }` なので、
パターンマッチは `located_expr.expression` 経由でアクセスする形になる。

## 進捗

- [x] Step 1: tree_parser への LocatedExpression 導入
- [x] Step 2: semantic_analyzer への LocatedExecExpression 導入
- [x] Step 3: compiler_ws / interpreter の対応
- [x] Step 4: テスト修正・検証
