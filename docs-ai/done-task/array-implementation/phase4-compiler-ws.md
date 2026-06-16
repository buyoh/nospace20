# Phase 4: Whitespace コンパイラ (compiler_ws) の変更

## 概要

Whitespace コンパイラの配列対応。メモリレイアウトの変更と、
`ExecExpression::ArrayAccess` のコード生成を実装する。

## 変更ファイル

- `src/compiler_ws/memory.rs` — メモリレイアウト
- `src/compiler_ws/context.rs` — コンテキスト
- `src/compiler_ws/expression.rs` — 式のコード生成
- `src/compiler_ws/statement.rs` — 文のコード生成（配列初期化）

## 1. メモリレイアウトの変更

### allocate_global の拡張

変更前:
```rust
pub fn allocate_global(&mut self) -> HeapAddress {
    let addr = Self::GLOBAL_PTR.offset(self.global_var_count);
    self.global_var_count += 1;
    addr
}
```

変更後:
```rust
/// グローバル変数を登録し、先頭アドレスを返す
/// size: スロット数（通常変数は 1、配列は配列サイズ）
pub fn allocate_global_slots(&mut self, size: i64) -> HeapAddress {
    let addr = Self::GLOBAL_PTR.offset(self.global_var_count);
    self.global_var_count += size;
    addr
}

/// 後方互換: 1スロットのグローバル変数を登録
pub fn allocate_global(&mut self) -> HeapAddress {
    self.allocate_global_slots(1)
}
```

### ローカル変数のサイズ

ローカル変数は `variable_count` （= スロット総数）で確保されるため、
Phase 2 で `variable_count` の計算が配列サイズを考慮するように変更されれば、
ローカルヒープの確保は自動的に対応される。

## 2. コンテキストの変更

### VarInfo の拡張

配列変数の場合、ベースオフセットからインデックスを加算してアクセスする。
`VarInfo` 自体の変更は不要（offset がベースを指す）。

### enter_function のサイズ計算

`enter_function` は `local_var_count` を受け取るが、
これは `scope.variable_count` から来る。Phase 2 での変更があれば対応不要。

## 3. 式のコード生成 (expression.rs)

### ArrayAccess のコード生成

`ExecExpression::ArrayAccess(var_ref, index_expr, _array_size)` の処理:

```rust
ExecExpression::ArrayAccess(var_ref, index_expr, _array_size) => {
    let var_info = ctx.get_var_info(var_ref);
    let mut prog = WsProgram::new();

    // アドレス計算: base_addr + index
    match var_info.scope {
        VarScope::Global => {
            // global_addr = GLOBAL_PTR + offset + index
            let base_addr = heap_layout::GLOBAL_PTR + var_info.offset;
            prog.push(Instruction::Push(WsNumber(base_addr)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
            prog.push(Instruction::Retrieve);
        }
        VarScope::Local => {
            // local_addr = heap[LOCAL_HEAP_BEGIN] + offset + index
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
            prog.push(Instruction::Retrieve);
        }
    }

    Ok(prog)
}
```

### 配列要素への代入

`Assign` のコード生成で、左辺が `ArrayAccess` のケースを追加:

```rust
// arr[i] = value
ExecExpression::ArrayAccess(var_ref, index_expr, _) => {
    let var_info = ctx.get_var_info(var_ref);
    let mut prog = WsProgram::new();

    // アドレスをスタックに積む
    match var_info.scope {
        VarScope::Global => {
            let base_addr = heap_layout::GLOBAL_PTR + var_info.offset;
            prog.push(Instruction::Push(WsNumber(base_addr)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
        }
        VarScope::Local => {
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
        }
    }

    // 値をスタックに積む
    prog.append(generate_expression(ctx, right)?);

    // Store
    prog.push(Instruction::Store);

    Ok(prog)
}
```

**注意**: Whitespace の `Store` 命令はスタックから `[address, value]` を取り、
`heap[address] = value` とする。スタック順に注意。

### &arr[i] のコード生成

`Operator1::Ref` のケースで `ArrayAccess` を処理:

```rust
Operator1::Ref => {
    match inner {
        ExecExpression::Variable(var_ref) => {
            // 既存: 変数のアドレスを返す
        }
        ExecExpression::ArrayAccess(var_ref, index_expr, _) => {
            // base_addr + index をスタックに積む
            let var_info = ctx.get_var_info(var_ref);
            let mut prog = WsProgram::new();
            match var_info.scope {
                VarScope::Global => {
                    let base_addr = heap_layout::GLOBAL_PTR + var_info.offset;
                    prog.push(Instruction::Push(WsNumber(base_addr)));
                    prog.append(generate_expression(ctx, index_expr)?);
                    prog.push(Instruction::Add);
                }
                VarScope::Local => {
                    prog.push(Instruction::Push(WsNumber(var_info.offset)));
                    prog.append(generate_expression(ctx, index_expr)?);
                    prog.push(Instruction::Add);
                    prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
                    prog.push(Instruction::Retrieve);
                    prog.push(Instruction::Add);
                }
            }
            Ok(prog)
        }
        _ => unimplemented!("reference of non-variable")
    }
}
```

## 4. 境界チェックについて

Whitespace コンパイラでは境界チェックを**省略する**。

理由:
- Whitespace の命令セットには条件分岐しかなく、境界チェックのコード量が大きい
- Whitespace プログラムのサイズを小さく保つ
- インタプリタで十分にテストされたコードをコンパイルする想定

## 5. テスト項目

### compile_test

- 配列宣言・アクセスの Whitespace コンパイル出力を検証
- 配列操作を含む統合テスト（Whitespace インタプリタで実行）

## 6. 考慮事項

### Ref/Deref 未実装

`compiler_ws/expression.rs` では `Operator1::Ref` は `unimplemented!()` となっている。
配列の `&` は Ref の実装後に対応。配列自体のコンパイルは Ref なしでも動作する。

### global_heap_size

`global_heap_size` は `scope.variable_count` を返す。Phase 2 により配列サイズを含むようになる。
