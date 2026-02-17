# Phase 1: コンパイラ変更設計

## 目標

`--std-ext debug` 指定時、`__trace`/`__assert`/`__assert_not` を Whitespace の負ヒープアドレスへの Store 命令として生成する。

## 変更対象ファイル

| ファイル | 変更内容 | 規模 |
|---|---|---|
| `src/compiler_ws/mod.rs` | `compile()` に拡張フラグを渡す | 小 |
| `src/compiler_ws/context.rs` | `CodeGenContext` に `debug_ext` フラグ追加 | 小 |
| `src/compiler_ws/expression.rs` | デバッグ組み込み関数のコード生成分岐 | 中 |
| `src/compiler_ws/memory.rs` | 拡張 API アドレス定数定義 | 小 |

## 詳細設計

### 1. `memory.rs`: 拡張 API アドレス定数

```rust
// 拡張 API 用負ヒープアドレス (--std-ext debug)
pub const EXT_TRACE_ADDR: HeapAddress = HeapAddress(-10);
pub const EXT_ASSERT_ADDR: HeapAddress = HeapAddress(-11);
pub const EXT_ASSERT_NOT_ADDR: HeapAddress = HeapAddress(-12);
```

### 2. `context.rs`: `CodeGenContext` に拡張フラグ追加

```rust
pub struct CodeGenContext<'a> {
    // ... 既存フィールド ...
    /// デバッグ拡張 API が有効か (--std-ext debug)
    debug_ext: bool,
}
```

- `new()` で `debug_ext: false` をデフォルト
- `debug_ext` を引数で受け取る `new_with_options()` または `new()` のシグネチャ変更
- `enter_function()` で `debug_ext` を子コンテキストに伝搬
- `pub fn is_debug_ext(&self) -> bool` アクセサ追加

### 3. `mod.rs`: `compile()` の拡張

```rust
/// オプション付きコンパイル
pub fn compile_with_options(
    scope: &Scope,
    debug_ext: bool,
) -> Result<WsProgram, CompileError> {
    let mut ctx = CodeGenContext::new_with_options(scope, debug_ext);
    // ... 既存の処理 ...
}

/// 従来互換 (debug_ext=false)
pub fn compile(scope: &Scope) -> Result<WsProgram, CompileError> {
    compile_with_options(scope, false)
}
```

### 4. `expression.rs`: デバッグ組み込み関数のコード生成

`generate_function_call` で `debug_ext` に応じて分岐:

```rust
BuiltinFunctionKind::Trace => {
    if ctx.is_debug_ext() {
        generate_builtin_debug_store(ctx, args, heap_layout::EXT_TRACE_ADDR)
    } else {
        generate_builtin_debug_noop(ctx, args)
    }
}
BuiltinFunctionKind::Assert => {
    if ctx.is_debug_ext() {
        generate_builtin_debug_store(ctx, args, heap_layout::EXT_ASSERT_ADDR)
    } else {
        generate_builtin_debug_noop(ctx, args)
    }
}
BuiltinFunctionKind::AssertNot => {
    if ctx.is_debug_ext() {
        generate_builtin_debug_store(ctx, args, heap_layout::EXT_ASSERT_NOT_ADDR)
    } else {
        generate_builtin_debug_noop(ctx, args)
    }
}
// __clog は常に noop (whitespace に対応する出力方式がない)
BuiltinFunctionKind::Clog => generate_builtin_debug_noop(ctx, args),
```

### 5. `generate_builtin_debug_store` の実装

`__trace(n)`, `__assert(n)`, `__assert_not(n)` 共通のコード生成。
引数を評価し、その値を返しつつ、指定された負ヒープアドレスに Store する。

```rust
fn generate_builtin_debug_store(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
    addr: HeapAddress,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    if args.is_empty() {
        // 引数なし: 0 を Store して 0 を返す
        prog.push(Instruction::Push(WsNumber(addr.0)));
        prog.push(Instruction::Push(WsNumber(0)));
        prog.push(Instruction::Store);
        prog.push(Instruction::Push(WsNumber(0)));
    } else {
        // 最初の引数を評価 → スタック: [..., val]
        prog.append(generate_expression(ctx, &args[0])?);

        // 値を複製（戻り値用） → スタック: [..., val, val]
        prog.push(Instruction::Duplicate);

        // アドレスをプッシュ → スタック: [..., val, val, addr]
        prog.push(Instruction::Push(WsNumber(addr.0)));

        // swap → スタック: [..., val, addr, val]
        prog.push(Instruction::Swap);

        // store: heap[addr] = val → スタック: [..., val]
        prog.push(Instruction::Store);

        // 残りの引数を評価して破棄（副作用のため）
        for arg in &args[1..] {
            prog.append(generate_expression(ctx, arg)?);
            prog.push(Instruction::Discard);
        }
    }

    Ok(prog)
}
```

**スタック操作の検証**:

```
1. evaluate arg[0]      → stack: [val]
2. Duplicate             → stack: [val, val]
3. Push(addr)            → stack: [val, val, addr]
4. Swap                  → stack: [val, addr, val]
5. Store                 → heap[addr] = val, stack: [val]  ✓ 戻り値 val がスタックに残る
```

## 注意事項

- `__clog` は `--std-ext debug` 有効時でも noop のまま。whitespace には対応する出力機構がない。
- `HeapAddress` の `.0` フィールドアクセスが `pub` であることを確認する（`types.rs` で `HeapAddress(pub i64)` の定義確認が必要）。
