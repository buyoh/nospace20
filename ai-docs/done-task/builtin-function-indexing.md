# ExecExpression::Function のインデックス化実装完了

実装日: 2026-02-11

## 概要

[symbol-table-design.md](../task/symbol-table-design.md) のステップ3「ExecExpression::Function のインデックス化」を実装しました。これにより、組み込み関数の識別が文字列マッチングから enum ベースに変更され、ランタイムでの文字列比較が不要になりました。

## 実装内容

### 1. BuiltinFunctionKind enum の定義

[src/semantic_analyzer/types.rs](../../src/semantic_analyzer/types.rs) に新しい enum を追加:

```rust
/// 組み込み関数の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunctionKind {
    /// __puti(x) - 整数を10進数で出力
    Puti,
    /// __putc(x) - 文字を出力
    Putc,
    /// __geti() - 整数を入力
    Geti,
    /// __getc() - 文字を入力
    Getc,
    /// __clog(x) - デバッグログ出力
    Clog,
    /// __assert(x) - x が非ゼロであることをアサート
    Assert,
    /// __assert_not(x) - x がゼロであることをアサート
    AssertNot,
    /// __trace(x) - 実行回数をトレース
    Trace,
}
```

### 2. ExecExpression の変更

`ExecExpression::BuiltinFunction` の型を `String` から `BuiltinFunctionKind` に変更:

```rust
pub(crate) enum ExecExpression {
    // ...
    /// 組み込み関数呼び出し
    /// Phase 6: 組み込み関数は BuiltinFunctionKind enum で識別
    BuiltinFunction(BuiltinFunctionKind, Vec<Box<ExecExpression>>),
    // ...
}
```

### 3. semantic analyzer での関数呼び出し解決

[src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs) で文字列を `BuiltinFunctionKind` に変換:

```rust
let builtin_kind = match f.as_str() {
    "__puti" => Some(types::BuiltinFunctionKind::Puti),
    "__putc" => Some(types::BuiltinFunctionKind::Putc),
    "__geti" => Some(types::BuiltinFunctionKind::Geti),
    "__getc" => Some(types::BuiltinFunctionKind::Getc),
    "__clog" => Some(types::BuiltinFunctionKind::Clog),
    "__assert" => Some(types::BuiltinFunctionKind::Assert),
    "__assert_not" => Some(types::BuiltinFunctionKind::AssertNot),
    "__trace" => Some(types::BuiltinFunctionKind::Trace),
    _ => None,
};

if let Some(kind) = builtin_kind {
    Ok(Box::new(ExecExpression::BuiltinFunction(kind, args)))
} else {
    // ユーザー定義関数として処理
}
```

### 4. interpreter の更新

[src/interpreter/exec.rs](../../src/interpreter/exec.rs) の `interpret_call_function` を enum ベースに変更:

```rust
fn interpret_call_function(
    &mut self,
    kind: &crate::semantic_analyzer::BuiltinFunctionKind,
    args: &Vec<Box<ExecExpression>>,
) -> ExpressionFlow {
    use crate::semantic_analyzer::BuiltinFunctionKind;

    match kind {
        BuiltinFunctionKind::Clog => { /* ... */ }
        BuiltinFunctionKind::Assert => { /* ... */ }
        BuiltinFunctionKind::Puti => { /* ... */ }
        // ... 他の組み込み関数
    }
}
```

文字列ベースの `interpret_call_user_function(id: &String, ...)` も削除しました。

### 5. compiler_ws の更新

[src/compiler_ws/expression.rs](../../src/compiler_ws/expression.rs) も同様に enum ベースに変更:

```rust
fn generate_function_call(
    ctx: &mut CodeGenContext,
    kind: &crate::semantic_analyzer::BuiltinFunctionKind,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    use crate::semantic_analyzer::BuiltinFunctionKind;

    match kind {
        BuiltinFunctionKind::Puti => generate_builtin_puti(ctx, args),
        BuiltinFunctionKind::Putc => generate_builtin_putc(ctx, args),
        // ...
    }
}
```

`CompileError::UndefinedFunction` バリアントは不要になりましたが、互換性のため残しています（警告が出ますが無害）。

## テスト

### 新規追加テスト

[resources/tests/passes/builtins/builtin_all_test.ns](../../resources/tests/passes/builtins/builtin_all_test.ns) を追加:

```nospace
func: main() {
    __puti(123);
    __putc('\n');
    __clog(42);
    __assert(1);
    __assert_not(0);
    __trace(999);
    return: 0;
}
```

### テスト結果

すべてのテストがパスしました:
- `cargo test --lib`: 158 テスト全てパス
- `cargo test --test code_test`: 109 テスト全てパス
- `cargo test --test compile_test`: 1 テスト全てパス

## パフォーマンス向上

この変更により、組み込み関数呼び出しのたびに行われていた文字列比較が、定数時間の enum マッチングに置き換えられました。

**変更前**: 文字列比較（最大 8 回の比較）
**変更後**: enum パターンマッチ（O(1)、コンパイラが最適化）

## 次のステップ

[symbol-table-design.md](../task/symbol-table-design.md) のステップ4:
- (4) `Scope.identifier_map` の縮小
- (5) `function_static_storage` のインデックスキー化
- (6) SymbolTable の導入

これらは関数名の完全なインデックス化とシンボルテーブルの集約を含みます。

## 影響を受けるファイル

- `src/semantic_analyzer/types.rs`: `BuiltinFunctionKind` 定義、`ExecExpression` 変更
- `src/semantic_analyzer/mod.rs`: 関数呼び出し解決の変更、`BuiltinFunctionKind` の公開
- `src/interpreter/exec.rs`: `interpret_call_function` の変更、不要な関数の削除
- `src/compiler_ws/expression.rs`: `generate_function_call` の変更
- `resources/tests/passes/builtins/builtin_all_test.ns`: 新規テスト追加

## 関連ドキュメント

- [symbol-table-design.md](../task/symbol-table-design.md): 全体設計
- [variable-identifier-to-slot-index.md](./variable-identifier-to-slot-index.md): ステップ2（完了）
- [function-args-identifier-resolution-completed.md](./function-args-identifier-resolution-completed.md): ステップ1（完了）
