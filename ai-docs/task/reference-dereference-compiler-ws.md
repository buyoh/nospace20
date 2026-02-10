# 参照・デリファレンス - Whitespace コンパイラ実装 (Phase 4)

## ステータス

- 作成日: 2026-02-10
- 状態: 未実装

## 背景

参照・デリファレンス演算子の実装は Phase 1-3 (token_parser, tree_parser, semantic_analyzer, interpreter) が完了しています。インタプリタでの実行は可能ですが、Whitespace へのコンパイルは未実装です。

完了レポート: [reference-dereference-interpreter-implementation.md](../done-task/reference-dereference-interpreter-implementation.md)

## 未実装の内容

### 現在の状態

`src/compiler_ws/expression.rs` において、`Operator1::Ref` と `Operator1::Deref` は `unimplemented!()` のままです:

```rust
Operator1::Ref => {
    // 未実装
    unimplemented!("reference operator (&) is not implemented yet")
}
Operator1::Deref => {
    // 未実装
    unimplemented!("dereference operator (*) is not implemented yet")
}
```

### 必要な実装

#### 1. `&var` (参照取得)

変数のヒープアドレス整数値をスタックに Push する。

**設計方針**:
- Whitespace はヒープベースのアーキテクチャ
- 変数は全てヒープアドレスで管理されている
- グローバル変数とローカル変数でアドレス計算方法が異なる

**実装例**:
```rust
Operator1::Ref => {
    match inner {
        ExecExpression::Variable(id_ref) => {
            // 変数のヒープアドレスを取得
            let heap_addr = ctx.get_variable_address(id_ref);
            // アドレスをスタックに Push
            prog.push(Instruction::Push(heap_addr));
        }
        ExecExpression::ArrayAccess(id_ref, index_expr, _) => {
            // 配列の場合: base_addr + index
            let base_addr = ctx.get_variable_address(id_ref);
            prog.push(Instruction::Push(base_addr));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
        }
        _ => {
            return Err(CompileError::InvalidReferenceTarget);
        }
    }
    Ok(prog)
}
```

#### 2. `*ptr` (デリファレンス読み取り)

スタックトップの値をアドレスとして `Retrieve` 命令を実行する。

**実装例**:
```rust
Operator1::Deref => {
    // 内部式を評価してアドレスをスタックに積む
    prog.append(generate_expression(ctx, inner)?);
    // スタックトップのアドレスから値を取得
    prog.push(Instruction::Retrieve);
    Ok(prog)
}
```

#### 3. `*ptr = value` (デリファレンス代入)

`generate_assignment` 関数を拡張して、左辺が `Operator1::Deref` の場合を処理する。

**現在の実装**:
```rust
fn generate_assignment(
    ctx: &mut CodeGenContext,
    left: &ExecExpression,
    right: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    match left {
        ExecExpression::Variable(id_ref) => {
            // 通常の変数代入
            let heap_addr = ctx.get_variable_address(id_ref);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Store(heap_addr));
        }
        // ... ArrayAccess の処理 ...
        _ => return Err(CompileError::InvalidLeftValue),
    }
    Ok(prog)
}
```

**拡張後**:
```rust
match left {
    ExecExpression::Variable(id_ref) => {
        // 通常の変数代入
        // ...
    }
    ExecExpression::Operation1(Operator1::Deref, addr_expr) => {
        // デリファレンス代入
        prog.append(generate_expression(ctx, addr_expr)?);  // アドレスをスタックに積む
        prog.append(generate_expression(ctx, right)?);       // 値をスタックに積む
        prog.push(Instruction::Store);                       // Store(アドレスなし版)
    }
    // ... ArrayAccess の処理 ...
    _ => return Err(CompileError::InvalidLeftValue),
}
```

### 技術的詳細

#### 変数アドレスの取得

`CodeGenContext` に変数のヒープアドレスを取得するメソッドが必要:

```rust
impl CodeGenContext {
    pub fn get_variable_address(&self, id_ref: &IdentifierRef) -> HeapAddress {
        if id_ref.is_global {
            // グローバル変数のアドレス
            self.memory_layout.global_address(id_ref.index)
        } else {
            // ローカル変数のアドレス
            self.memory_layout.local_address(id_ref.scope_depth, id_ref.index)
        }
    }
}
```

このメソッドは既存の `MemoryLayout` を利用して実装可能。

## テスト計画

### ユニットテスト

`src/compiler_ws/expression.rs` に追加:
- `&var` の単純な参照取得
- `*ptr` の単純なデリファレンス
- `*ptr = value` のデリファレンス代入

### 統合テスト

既存の参照・デリファレンステストを Whitespace コンパイラでも実行:
- `test_operators_ref_basic_001` - 基本的な参照・デリファレンス
- `test_operators_ref_deref_assign_001` - デリファレンス代入
- `test_operators_ref_double_001` - ダブルデリファレンス
- `test_operators_ref_func_arg_001` - 関数引数として参照を渡す
- `test_operators_ref_swap_001` - 参照を使った swap 関数

現在、これらのテストは `mode: InterpretFunc` のみで実行されています。Phase 4 完了後は `mode: CompileWs` でも実行できるようにします。

## 実装手順

1. `CodeGenContext::get_variable_address` メソッドを実装（または既存メソッドを確認）
2. `Operator1::Ref` の実装
   - 変数の場合
   - 配列要素の場合（将来的に）
3. `Operator1::Deref` の実装（読み取り）
4. `generate_assignment` の拡張（デリファレンス代入）
5. ユニットテストの追加
6. 統合テストの mode を `CompileWs` に拡張
7. 全テストスイート（`cargo test`）が PASS することを確認

## 依存関係

- **MemoryLayout**: 変数のヒープアドレス計算に使用
- **Instruction**: `Push`, `Retrieve`, `Store` 命令の定義
- **generate_expression**: 式のコード生成（再帰的に使用）
- **generate_assignment**: 代入のコード生成（拡張が必要）

## 関連ドキュメント

- [compiler-ws.md](./compiler-ws.md) - Whitespace コンパイラ実装設計（詳細）
- [overview.md](./overview.md) - 参照・デリファレンス全体設計
- [../done-task/reference-dereference-interpreter-implementation.md](../done-task/reference-dereference-interpreter-implementation.md) - Phase 1-3 完了レポート

## 備考

Whitespace コンパイラでの実装は、インタプリタでの実装に比べて自然に行えます。Whitespace が元々ヒープベースのアーキテクチャであり、変数が全てヒープアドレスで管理されているためです。

インタプリタでは `Vec<i64>` に整数としてアドレスをエンコードする必要がありましたが、Whitespace では既にアドレスがスタック上で整数として扱われているため、追加の抽象化は不要です。
