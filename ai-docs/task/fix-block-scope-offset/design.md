# Bug D: 修正設計

## 方針

**コンパイラ側のみの修正**で、`CodeGenContext` にスコープオフセットスタックを導入する。
セマンティック分析器やインタプリタへの変更は不要。

ブロックスコープの変数を関数ローカルヒープ内で一意なオフセットに配置するため:
1. 関数内の全ネストブロックの変数合計数を事前計算し、ローカルヒープサイズとする
2. `CodeGenContext` にスコープオフセットスタックを持たせ、`get_var_info` で `scope_depth` を考慮する
3. `generate_block` でブロック進入/退出時にスコープオフセットを操作する

## 修正後のメモリレイアウト例 (qsort main)

```
ヒープオフセット  変数
───────────────  ──────────────
LHB + 0..19     arr[0..19]     (関数スコープ)
LHB + 20        n              (関数スコープ)
LHB + 21        i (2番目)      (関数スコープ)
LHB + 22        i (ブロック内)  (内部ブロックスコープ) ← 衝突しない
```

total_var_count = 22 (関数スコープ) + 1 (ブロック) = 23

## 変更対象モジュール

### 1. `src/compiler_ws/context.rs` — CodeGenContext 拡張

#### フィールド追加

```rust
pub struct CodeGenContext<'a> {
    // ... 既存フィールド ...

    /// スコープオフセットスタック
    /// 各エントリは、そのスコープの変数のヒープ内ベースオフセット
    /// 末尾が現在のスコープ
    scope_offsets: Vec<i64>,

    /// 次のブロックスコープに割り当てる開始オフセット
    next_var_offset: i64,
}
```

#### `new()` 修正

```rust
pub fn new(scope: &'a Scope) -> Self {
    Self {
        // ... 既存 ...
        scope_offsets: vec![0],
        next_var_offset: 0,
    }
}
```

#### `enter_function()` 修正

```rust
pub fn enter_function(
    &self,
    total_var_count: usize,
    func_scope_var_count: usize,
) -> CodeGenContext<'a> {
    CodeGenContext {
        // ... 既存 ...
        local_heap_size: total_var_count as i64,
        scope_offsets: vec![0],  // 関数スコープはオフセット 0
        next_var_offset: func_scope_var_count as i64,
        // ...
    }
}
```

#### 新規メソッド `enter_block_scope()` / `leave_block_scope()`

```rust
/// ブロックスコープに入る
/// block_var_count: このブロックの variable_count
pub fn enter_block_scope(&mut self, block_var_count: usize) {
    self.scope_offsets.push(self.next_var_offset);
    self.next_var_offset += block_var_count as i64;
}

/// ブロックスコープから出る
pub fn leave_block_scope(&mut self) {
    self.scope_offsets.pop();
    // next_var_offset は戻さない（各スコープに一意のオフセットを保証）
}
```

#### `get_var_info()` 修正

```rust
pub fn get_var_info(&self, var_ref: &IdentifierRef) -> VarInfo {
    if var_ref.is_global {
        VarInfo {
            scope: VarScope::Global,
            offset: var_ref.local_index as i64,
        }
    } else {
        let scope_idx = self.scope_offsets.len() - 1 - var_ref.scope_depth;
        let base_offset = self.scope_offsets[scope_idx];
        VarInfo {
            scope: VarScope::Local,
            offset: base_offset + var_ref.local_index as i64,
        }
    }
}
```

### 2. `src/compiler_ws/statement.rs` — ブロック/関数生成の修正

#### 関数の全変数合計数を計算するヘルパー関数群を追加

```rust
/// 関数内の全ブロック（ネスト含む）の変数合計数を計算
fn calculate_total_variable_count(block: &Block) -> usize {
    block.scope.variable_count
        + count_nested_vars_in_statements(&block.statements)
}

fn count_nested_vars_in_statements(stmts: &[ExecStatement]) -> usize {
    stmts.iter().map(count_nested_vars_in_statement).sum()
}

fn count_nested_vars_in_statement(stmt: &ExecStatement) -> usize {
    match stmt {
        ExecStatement::Expression(expr) | ExecStatement::Return(expr) => {
            count_nested_vars_in_expression(expr)
        }
        ExecStatement::Break | ExecStatement::Continue => 0,
    }
}

fn count_nested_vars_in_expression(expr: &ExecExpression) -> usize {
    match expr {
        ExecExpression::If(cond, then_block, else_block) => {
            count_nested_vars_in_expression(cond)
                + calculate_total_variable_count(then_block)
                + calculate_total_variable_count(else_block)
        }
        ExecExpression::While(cond, body) => {
            count_nested_vars_in_expression(cond)
                + calculate_total_variable_count(body)
        }
        ExecExpression::Block(block) => {
            calculate_total_variable_count(block)
        }
        ExecExpression::Operation1(_, inner) => {
            count_nested_vars_in_expression(inner)
        }
        ExecExpression::Operation2(_, l, r) => {
            count_nested_vars_in_expression(l)
                + count_nested_vars_in_expression(r)
        }
        ExecExpression::BuiltinFunction(_, args)
        | ExecExpression::UserFunction(_, args) => {
            args.iter()
                .map(|a| count_nested_vars_in_expression(a))
                .sum()
        }
        ExecExpression::ArrayAccess(_, index_expr, _) => {
            count_nested_vars_in_expression(index_expr)
        }
        ExecExpression::Variable(_) | ExecExpression::Factor(_) => 0,
    }
}
```

#### `generate_function_definition()` 修正

```rust
// 変更前:
let local_var_count = func.block.scope.variable_count;
let mut local_ctx = ctx.enter_function(local_var_count);

// 変更後:
let func_scope_var_count = func.block.scope.variable_count;
let total_var_count = calculate_total_variable_count(&func.block);
let mut local_ctx = ctx.enter_function(total_var_count, func_scope_var_count);
```

#### `generate_block()` 修正

```rust
pub fn generate_block(
    ctx: &mut CodeGenContext,
    block: &Block,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // ブロックスコープのオフセットを設定
    ctx.enter_block_scope(block.scope.variable_count);

    for stmt in &block.statements {
        prog.append(generate_statement(ctx, stmt)?);
    }

    // ブロックスコープから退出
    ctx.leave_block_scope();

    prog.push(Instruction::Push(WsNumber(0)));
    Ok(prog)
}
```

## 変更しないモジュール

- `src/semantic_analyzer/` — 変更不要。`IdentifierRef.scope_depth` と `local_index` の意味は不変
- `src/interpreter/` — 変更不要。独立した `scope_stack` 方式で正しく動作している
- `src/compiler_ws/expression.rs` — 変更不要。`generate_if_expression`, `generate_while_expression`,
  `ExecExpression::Block` のいずれも `generate_block()` に委譲しており、
  `generate_block` 内でスコープ操作が行われるため自動的に対応

## 動作検証

### 修正後の qsort main() 実行トレース

1. `enter_function(total=23, func_scope=22)` → scope_offsets=[0], next=22
2. 内部ブロック `{` → `enter_block_scope(1)` → scope_offsets=[0, 22], next=23
3. `let: i(0)`: scope_depth=0, local_index=0 → offset = scope_offsets[1] + 0 = **22** (arr[0] と衝突しない)
4. `arr[i]`: arr は scope_depth=1, local_index=i → offset = scope_offsets[0] + i (正しい)
5. 全入力値が正しく配列に格納される → qsort → 正しい出力

### 影響テスト (2026-02-17 更新)

Bug D の修正により以下の 13 テストが成功に変わることが期待される:

| テスト名 | 失敗パターン |
|----------|-------------|
| test_example_qsort_ws_self | 出力不一致（"0 0 0 1 1 4 7 "） |
| test_ok_coding_c004_ws_self | AssertionFailed |
| test_scope_block_expr_basic_001_ws_self | AssertionFailed |
| test_scope_block_expr_parent_scope_001_ws_self | AssertionFailed |
| test_scope_block_var_no_collision_001_ws_self | 出力不一致 |
| test_scope_disabled_scope_block_var_001_ws_self | AssertionFailed |
| test_scope_scope_block_001_ws_self | AssertionFailed |
| test_scope_scope_nested_blocks_001_ws_self | AssertionFailed |
| test_scope_scope_shadow_multi_001_ws_self | AssertionFailed |
| test_scope_scope_shadowing_002_ws_self | AssertionFailed |
| test_literals_comment_japanese_001_ws_self | AssertionFailed |
| test_scope_block_expr_nested_001_ws_self | AssertionFailed (Bug D + 式の値返却) |
| test_scope_block_expr_value_001_ws_self | AssertionFailed (Bug D + 式の値返却) |

**注**: block_expr_nested_001 と block_expr_value_001 は Bug D に加えてブロック式の値返却問題もあるため、
Bug D だけの修正では不十分な可能性あり。

### Bug D 修正では解決しないテスト (5件)

以下のテストは Bug D とは別の根本原因を持つ:

| テスト名 | 原因 |
|----------|------|
| test_control_flow_if_expr_value_001_ws_self | if/while 式の値返却が WS コンパイラ未実装 |
| test_control_flow_while_expr_value_001_ws_self | 同上 |
| test_scope_scope_static_mixed_001_ws_self | static 変数 + ネスト関数のスコープ問題 |
| test_scope_scope_static_multi_decl_001_ws_self | 同上 |
| test_scope_scope_static_nested_001_ws_self | 同上 |

### テスト計画

1. Bug D 修正後、上記 13 テスト（11件確実 + 2件条件付き）を検証
2. 既存の成功テスト (259件) が引き続き成功すること
3. 他のテストスイート（unit, compile_test, large tests）が引き続き成功すること

### リスク評価

- **低リスク**: 内部ブロックスコープを使わないテストでは `generate_block` が
  `enter_block_scope(0)` を呼ぶだけで、変数オフセットの計算結果は従来と同一
- **正方向の影響**: 従来たまたま衝突が起きなかったケースでも、
  今後正しくメモリが分離されるため、潜在バグの予防になる
