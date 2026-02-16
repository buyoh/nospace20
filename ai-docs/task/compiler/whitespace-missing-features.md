# Whitespace コンパイラの未実装機能

## 概要

`src/compiler_ws/` モジュールの Whitespace コンパイラは多くの機能が実装済みですが、以下の機能が未実装です。

## 未実装機能一覧

### 1. ユーザー定義関数呼び出し

**状態**: ❌ 未実装

**affected files**:
- `src/compiler_ws/expression.rs` (line 47)

**説明**:
- `ExecExpression::UserFunction` のコード生成が未実装
- 現在はエラーを返す: `"user-defined function calls are not yet supported in compiler"`
- インタプリタでは実装済み (`src/interpreter/exec.rs`)

**実装に必要なこと**:
```rust
// expression.rs の generate_expression() 内
ExecExpression::UserFunction(func_ref, args) => {
    // 1. 引数を評価してスタックにプッシュ（逆順）
    // 2. 関数ラベルを取得
    // 3. Call 命令を生成
    // 4. 戻り値がスタックに残る
}
```

**参考実装**:
- インタプリタの実装: `src/interpreter/exec.rs::interpret_call_user_function_by_ref()`
- 関数定義の生成: `src/compiler_ws/statement.rs::generate_function_definition()`
- 引数処理のロジックは既に `generate_function_definition()` に存在

**理由**:
- 現在のコンパイラは main 関数のみを対象としている
- 複数の関数定義・呼び出しのサポートには追加のラベル管理が必要

---

### 2. break 文

**状態**: ❌ 未実装

**affected files**:
- `src/compiler_ws/statement.rs` (line 63)

**説明**:
- `ExecStatement::Break` のコード生成が未実装
- 現在はエラーを返す: `"break not implemented"`
- インタプリタでは実装済み

**実装に必要なこと**:
```rust
// statement.rs の generate_statement() 内
ExecStatement::Break => {
    // 1. ループ終了ラベルへ Jump 命令を生成
    // 2. ラベルはコンテキストで管理（ループのネスト対応）
}
```

**課題**:
- ループ終了ラベルをコンテキストで管理する仕組みが必要
- ネストしたループに対応するため、ラベルスタックを実装する必要がある
- `CodeGenContext` に `loop_label_stack: Vec<(LabelId, LabelId)>` を追加
  - 第1要素: loop_start, 第2要素: loop_end

**参考実装**:
- while 式の生成: `src/compiler_ws/expression.rs::generate_while_expression()`

---

### 3. continue 文

**状態**: ❌ 未実装

**affected files**:
- `src/compiler_ws/statement.rs` (line 66)

**説明**:
- `ExecStatement::Continue` のコード生成が未実装
- 現在はエラーを返す: `"continue not implemented"`
- インタプリタでは実装済み

**実装に必要なこと**:
```rust
// statement.rs の generate_statement() 内
ExecStatement::Continue => {
    // 1. ループ開始ラベルへ Jump 命令を生成
    // 2. ラベルはコンテキストで管理（ループのネスト対応）
}
```

**課題**:
- break と同様に、ループ開始ラベルをコンテキストで管理する仕組みが必要
- `CodeGenContext` のループラベルスタックを使用

**参考実装**:
- while 式の生成: `src/compiler_ws/expression.rs::generate_while_expression()`

---

## 実装状況まとめ

### ✅ 実装済み機能

| カテゴリ | 機能 |
|---------|------|
| 演算 | 四則演算、比較演算、論理演算（短絡評価なし） |
| 変数 | グローバル変数、ローカル変数 |
| 配列 | 宣言、アクセス、代入 |
| ポインタ | 参照演算子 (`&`)、参照解除演算子 (`*`) |
| 制御構造 | if 式、while 式、return 文 |
| 関数 | main 関数の定義と実行 |
| 組み込み関数 | `__puti`, `__putc`, `__geti`, `__getc`<br>`__clog`, `__trace`, `__assert`, `__assert_not` (無視) |
| ブロック | ブロックスコープ式 |

### ❌ 未実装機能

| 機能 | 状態 | 優先度 |
|------|------|--------|
| ユーザー定義関数呼び出し | 未実装 | 高 |
| break 文 | 未実装 | 中 |
| continue 文 | 未実装 | 中 |

### ⚠️ 制限事項

- WhitespaceVM には短絡評価がないため、論理演算は組み込みルーチンで実装
- 配列の境界チェックは行わない（Whitespace の命令セットの制約）
- デバッグ用組み込み関数は無視される（引数のみ評価）

---

## テストカバレッジ

現在、以下のテストでWhitespaceコンパイラがテストされています：

```bash
# test-manifest.yaml で targets: [interpreter, whitespace] が指定されているテスト
grep -A 2 "targets:.*whitespace" resources/tests/test-manifest.yaml | grep "name:" | wc -l
```

約21個のテストケースがWhitespaceターゲットでテストされています。
ユーザー定義関数、break、continueを使用するテストは現時点ではWhitespaceターゲットから除外されています。

---

## 実装計画

### Phase 7: ユーザー定義関数呼び出しのサポート

1. 関数ラベルの管理 (既に `CodeGenContext` に実装済み)
2. 関数呼び出しのコード生成
3. 複数関数の定義生成 (現在は main のみ)
4. テストケースの追加

### Phase 8: break/continue のサポート

1. `CodeGenContext` にループラベルスタックを追加
2. while 式生成時にラベルをプッシュ/ポップ
3. break/continue 文のコード生成
4. ネストしたループのテスト

---

## 関連ドキュメント

- [compiler-rust-impl/README.md](../spec/compiler-rust-impl/README.md) - Rust 実装設計
- [compiler-rust-impl/codegen.md](../spec/compiler-rust-impl/codegen.md) - コード生成の詳細
- [implementation-status.md](../spec/implementation-status.md) - 全体の実装状況
- `src/compiler_ws/` - 実装コード
- `src/interpreter/exec.rs` - インタプリタの参考実装
