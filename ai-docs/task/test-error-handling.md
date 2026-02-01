# テストのエラーハンドリング強化

## 完了状況

✅ **Phase 1完了**: 基本構造の整備
- ディレクトリ構造を `passes/` と `fails/syntax/` に分離
- check.json のスキーマを拡張（後方互換性あり）
- 既存のテストケースを `passes/` 配下に移行

✅ **Phase 2完了**: テスト関数の実装
- `test_ok_coding_base()` を更新してパスを `resources/tests/passes/` に変更
- `test_syntax_error_base()` を新規実装
- TestConfig enum を導入（serde でデシリアライズ）

✅ **Phase 3開始**: テストケースの追加
- `fails/syntax/` に3つのサンプルテストを追加
  - invalid_token_001: 不正なトークン（@）
  - unclosed_paren_001: 閉じ括弧がない
  - unexpected_eof_001: 予期しないEOF

## 現在の問題

⚠️ **既存テストの一部が失敗**（12/47件）
- **実際の原因**: ブロックスコープの変数が未実装（src/semantic_analyzer/mod.rs:184 で panic）
- エラーメッセージ: "todo: block scoped variable is not implemented"
- 影響を受けるテスト例: `control_flow/if_001`, `scope/scope_block_001`, `functions/func_args_001`, など
- この問題はテスト強化の前から存在していた

**調査済み**: 日本語コメント問題
- 一部のテストファイルに日本語コメントが含まれているが、トークナイザーは正常に処理している
- 検証結果: `operators/unary_001.ns`, `io/getc_basic_001.ns`, `functions/func_hoist_001.ns` など日本語コメントを含むテストは成功
- トークナイザーはUTF-8文字を含むコメント（`# ... #`形式）を正しくスキップできる
- **結論**: 日本語コメントはテスト失敗の原因ではない

## 次のステップ

### 優先度: 高
1. ブロックスコープ変数の実装
   - semantic_analyzerでブロックスコープの変数宣言をサポート
   - 失敗している12件のテストが通過するようになる

### 優先度: 中
2. 構文エラーテストケースの拡充
   - 様々な構文エラーパターンをカバー
   - エラーメッセージの内容検証（contains フィールドの活用）

3. 意味解析エラーのテスト準備
   - `fails/semantic/` ディレクトリの活用
   - 未定義変数、型エラーなどのテストケース

## 完了内容の詳細

### ディレクトリ構造

```
resources/tests/
  passes/           # 正常系テスト
    c000.ns, c001.ns, ...  # レガシーテスト
    literals/
    operators/
    builtins/
    variables/
    functions/
    control_flow/
    scope/
    integration/
  
  fails/            # エラー系テスト（新規）
    syntax/         # 構文解析エラー
      invalid_token_001.ns + .check.json
      unclosed_paren_001.ns + .check.json
      unexpected_eof_001.ns + .check.json
    semantic/       # 意味解析エラー（準備済み）
    runtime/        # 実行時エラー（準備済み）
```

### check.json の形式

#### 正常系（後方互換性あり）

```json
{
  "trace": [1, 2, 3]
}
```

または新形式：

```json
{
  "type": "success",
  "trace": [1, 2, 3]
}
```

#### 構文解析エラー

```json
{
  "type": "parse_error",
  "phase": "tokenize",  // or "tree"
  "error_count": 1,     // オプション（未実装）
  "contains": ["expected", "identifier"]  // オプション（未実装）
}
```

### テスト実装

- `TestConfig` enum を導入し、serde でデシリアライズ
- `test_ok_coding_base()`: 正常系テスト（パスを `passes/` に変更）
- `test_syntax_error_base()`: 構文エラーテスト（新規実装）
- マクロ `test_syntax_error!` を追加

### 依存関係の追加

Cargo.toml に serde を追加：
```toml
serde = { version = "1.0", features = ["derive"] }
```

## テスト結果

```bash
cargo test --test code_test
# 結果: 22 passed; 10 failed
```

成功しているテスト:
- レガシーテスト（c000, c001, c003, c004）
- literals（num, ident, comment）
- operators（一部）
- builtins（trace, assert）
- variables（var_basic, var_hoist）
- 構文エラーテスト（3件すべて成功）

## 利点

✅ エラーケースの網羅的なテスト基盤を構築
✅ 正常系と異常系の明確な分離
✅ 後方互換性を保持
✅ 拡張可能な設計（semantic, runtimeエラーにも対応可能）
