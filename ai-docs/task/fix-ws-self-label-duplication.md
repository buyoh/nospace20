# fix: compiler_ws ラベル ID 重複バグの修正

## 概要

`cargo test` で 15件の `_ws_self` テストが失敗している。
根本原因は `compiler_ws` のラベルアロケータにおけるラベル ID 重複バグ。
既存調査ドキュメント [whitespace-self-test-failures.md](whitespace-self-test-failures.md) で原因が特定済み。

本ドキュメントでは具体的なコード修正手順をまとめる。

## 失敗テスト一覧 (15件)

| # | テスト名 | 失敗パターン |
|---|---|---|
| 1 | test_example_puts_ws_self | Suspended (無限ループ) |
| 2 | test_example_fibonacci_ws_self | 出力不一致 |
| 3 | test_example_qsort_ws_self | 出力不一致 (空出力) |
| 4 | test_legacy_011_ws_self | IoError |
| 5 | test_legacy_012_ws_self | IoError |
| 6 | test_legacy_014_ws_self | 出力不一致 |
| 7 | test_legacy_015_ws_self | Suspended |
| 8 | test_legacy_020_ws_self | 出力不一致 |
| 9 | test_scope_func_shadowing_nested_001_ws_self | Suspended |
| 10 | test_scope_func_shadowing_siblings_001_ws_self | Suspended |
| 11 | test_scope_scope_nested_func_001_ws_self | Suspended |
| 12 | test_scope_scope_static_counter_factory_001_ws_self | Suspended |
| 13 | test_scope_scope_static_mixed_001_ws_self | Suspended |
| 14 | test_scope_scope_static_multi_decl_001_ws_self | Suspended |
| 15 | test_scope_scope_static_nested_001_ws_self | Suspended |

## 根本原因

`CodeGenContext::enter_function()` がラベルアロケータを `clone()` しているため、
子コンテキスト（関数本体）で割り当てたラベル ID が親コンテキストに反映されない。

結果として次の関数定義時に同じラベル ID が再利用され、Whitespace VM の HashMap ベースのラベル解決で
後の定義が先の定義を上書きし、制御フローが破壊される。

### 問題箇所 (2箇所)

1. **`src/compiler_ws/context.rs` L63** - `enter_function()` 内の `self.labels.clone()`
2. **`src/compiler_ws/statement.rs` L94** - `generate_return()` 内の `ctx.clone()`

## 修正計画

### ステップ 1: `LabelAllocator` に同期メソッド追加

**ファイル**: `src/compiler_ws/label.rs`

`LabelAllocator` に子アロケータから `next_id` を同期するメソッドを追加する。

```rust
/// 子アロケータで消費されたラベル ID を同期する。
/// 子アロケータの next_id が自身より大きい場合に更新する。
pub fn sync_next_id(&mut self, other: &LabelAllocator) {
    if other.next_id > self.next_id {
        self.next_id = other.next_id;
    }
}
```

### ステップ 2: `CodeGenContext` に同期メソッド追加

**ファイル**: `src/compiler_ws/context.rs`

```rust
/// 子コンテキストで消費されたラベルカウンタを親に同期する。
pub fn sync_labels_from(&mut self, child: &CodeGenContext) {
    self.labels.sync_next_id(&child.labels);
}
```

### ステップ 3: `generate_function_definition()` で同期呼び出し追加

**ファイル**: `src/compiler_ws/statement.rs`

`generate_function_definition()` 内、関数本体のコード生成後に `ctx.sync_labels_from(&local_ctx)` を呼ぶ。

```rust
fn generate_function_definition(
    ctx: &mut CodeGenContext,
    func_name: &str,
    func: &crate::semantic_analyzer::Function,
) -> Result<WsProgram, CompileError> {
    // ...
    let mut local_ctx = ctx.enter_function(local_var_count);
    // ... 関数本体コード生成 ...

    // ★追加: 子コンテキストのラベルカウンタを親に同期
    ctx.sync_labels_from(&local_ctx);

    // 関数定義終了ラベル
    prog.push(Instruction::Label(label.offset(1)));
    Ok(prog)
}
```

具体的には、`prog.push(Instruction::Return);` の行の後、
`prog.push(Instruction::Label(label.offset(1)));` の前に挿入する。

### ステップ 4: `generate_return()` のシグネチャ変更

**ファイル**: `src/compiler_ws/statement.rs`

`generate_return()` の引数を `&CodeGenContext` → `&mut CodeGenContext` に変更し、
`ctx.clone()` を不要にする。

変更前:
```rust
fn generate_return(
    ctx: &CodeGenContext,
    expr: &crate::semantic_analyzer::ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    prog.append(expression::generate_expression(&mut ctx.clone(), expr)?);
    // ...
}
```

変更後:
```rust
fn generate_return(
    ctx: &mut CodeGenContext,
    expr: &crate::semantic_analyzer::ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    prog.append(expression::generate_expression(ctx, expr)?);
    // ...
}
```

### ステップ 5: ユニットテスト追加

**ファイル**: `src/compiler_ws/label.rs`

`sync_next_id` のテストを追加:

```rust
#[test]
fn test_sync_next_id() {
    let mut parent = LabelAllocator::new();
    parent.allocate(); // next_id = 17
    let mut child = parent.clone();
    child.allocate(); // child next_id = 18
    child.allocate(); // child next_id = 19
    parent.sync_next_id(&child);
    assert_eq!(parent.allocate().0, 19); // 同期後は 19 から割り当て
}
```

## 影響範囲

### 変更ファイル

| ファイル | 変更内容 | リスク |
|---|---|---|
| `src/compiler_ws/label.rs` | `sync_next_id()` メソッド追加 | 低 (新規メソッド) |
| `src/compiler_ws/context.rs` | `sync_labels_from()` メソッド追加 | 低 (新規メソッド) |
| `src/compiler_ws/statement.rs` | 同期呼び出し追加 + `generate_return` シグネチャ変更 | 中 |

### リスク評価

- `sync_labels_from()` は新規メソッドであり、既存コードに影響しない
- `generate_return()` のシグネチャ変更は、呼び出し元が `generate_statement()` 内の1箇所のみで影響範囲が限定的
- `generate_statement()` は既に `ctx: &mut CodeGenContext` を受け取っているため、`generate_return` に `&mut` で渡すのは自然

### 検証方法

1. `cargo test` で全246テスト（現在15件失敗中）が通ること
2. 特に以下を確認:
   - 修正対象の15件 `_ws_self` テストがすべて成功
   - 既存113件の `ignored` テストの状態に変化がないこと

## ステータス

- [x] 原因調査完了
- [x] 修正計画策定完了
- [ ] 修正実装
- [ ] テスト全通過確認
