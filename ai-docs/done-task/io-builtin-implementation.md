# compiler_ws: I/O ビルトイン関数の実装設計

## 概要

`src/compiler_ws` において `__puti`、`__putc`、`__geti`、`__getc` の4つの I/O ビルトイン関数が未実装である。
Whitespace 言語には対応する I/O 命令が存在するため、これらを実装する設計をまとめる。

## 現状分析

### Whitespace I/O 命令（instruction.rs に定義済み）

| Instruction | Whitespace | スタック効果 | 説明 |
|-------------|-----------|-------------|------|
| `OutputNumber` | `TB LF SP TB` | `[..., n] → [...]` | 値を10進数として出力 |
| `OutputChar` | `TB LF SP SP` | `[..., c] → [...]` | 値をASCII文字として出力 |
| `InputNumber` | `TB LF TB TB` | `[..., addr] → [...]` | 整数を読み込み `heap[addr]` に格納 |
| `InputChar` | `TB LF TB SP` | `[..., addr] → [...]` | 文字を読み込み `heap[addr]` に格納 |

### nospace 仕様（docs/spec.md §3.2）

| 関数 | 引数 | 戻り値 | 説明 |
|------|------|--------|------|
| `__puti(x)` | 整数 x | x | 整数を10進数で標準出力に出力し、x を返す |
| `__putc(x)` | 整数 x | x | x をASCII文字として標準出力に出力し、x を返す |
| `__geti()` | なし | 入力値 | 標準入力から整数を読み込み、その値を返す |
| `__getc()` | なし | 入力値 | 標準入力から1文字を読み込み、ASCII値を返す |

### 未実装箇所

`expression.rs` の `generate_function_call` がすべての関数呼び出しに対して `Err(CompileError::UndefinedFunction(...))` を返している。

```rust
fn generate_function_call(
    _ctx: &mut CodeGenContext,
    func_name: &str,
    _args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    // TODO: 組み込み関数と ユーザー定義関数の実装
    Err(CompileError::UndefinedFunction(func_name.to_string()))
}
```

## 設計

### 方針: インライン展開

ビルトイン関数呼び出しを `generate_function_call` 内で検出し、対応する Whitespace 命令列を直接生成する。

**理由:**
- I/O 操作は数命令で実現でき、サブルーチン化するオーバーヘッドに見合わない
- 旧実装（C++ 版）も同様のインライン展開方式を採用していた（`io.md` に記録あり）
- ユーザー定義関数の Call/Return 機構を I/O のためだけに実装する必要がない

### 変更対象モジュール

| ファイル | 変更内容 |
|---------|---------|
| `src/compiler_ws/expression.rs` | `generate_function_call` にビルトイン関数の分岐を追加 |

他のファイルの変更は不要。`instruction.rs` には既に `OutputNumber`、`OutputChar`、`InputNumber`、`InputChar` が定義済み。`memory.rs` には `TEMP_PTR`（アドレス 4）が定義済み。

### 各関数のコード生成

#### `__puti(x)` — 整数出力

```
[引数 x の評価]     ; スタック: [..., x]
Duplicate            ; スタック: [..., x, x]
OutputNumber         ; x を出力, スタック: [..., x]
```

- 引数を評価してスタックに積む
- `Duplicate` で値を複製（戻り値として残すため）
- `OutputNumber` で出力（スタックから1つ消費）
- 結果: スタックトップに x が残る（= 戻り値）

#### `__putc(x)` — 文字出力

```
[引数 x の評価]     ; スタック: [..., x]
Duplicate            ; スタック: [..., x, x]
OutputChar           ; x を文字として出力, スタック: [..., x]
```

- `__puti` と同構造、`OutputNumber` の代わりに `OutputChar` を使用

#### `__geti()` — 整数入力

Whitespace の `InputNumber` は「スタックからアドレスをポップし、入力値をそのアドレスに格納」する命令であるため、一時ヒープアドレスを経由する必要がある。

```
Push(TEMP_PTR)       ; スタック: [..., 4]
Duplicate            ; スタック: [..., 4, 4]
InputNumber          ; heap[4] = 入力値, スタック: [..., 4]
Retrieve             ; スタック: [..., heap[4]]
```

- `TEMP_PTR`（= 4）をプッシュ
- `Duplicate` でアドレスを複製（`InputNumber` が1つ消費し、`Retrieve` がもう1つ使うため）
- `InputNumber` で入力値を `heap[TEMP_PTR]` に格納
- `Retrieve` で `heap[TEMP_PTR]` の値をスタックに取り出す

#### `__getc()` — 文字入力

```
Push(TEMP_PTR)       ; スタック: [..., 4]
Duplicate            ; スタック: [..., 4, 4]
InputChar            ; heap[4] = 入力文字, スタック: [..., 4]
Retrieve             ; スタック: [..., heap[4]]
```

- `__geti` と同構造、`InputNumber` の代わりに `InputChar` を使用

### 実装の擬似コード

`expression.rs` の `generate_function_call` を以下のように変更する:

```rust
fn generate_function_call(
    ctx: &mut CodeGenContext,
    func_name: &str,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    match func_name {
        "__puti" => generate_builtin_puti(ctx, args),
        "__putc" => generate_builtin_putc(ctx, args),
        "__geti" => generate_builtin_geti(ctx, args),
        "__getc" => generate_builtin_getc(ctx, args),
        _ => {
            // TODO: ユーザー定義関数の実装
            Err(CompileError::UndefinedFunction(func_name.to_string()))
        }
    }
}

fn generate_builtin_puti(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    // 引数を評価
    prog.append(generate_expression(ctx, &args[0])?);
    // 値を複製（戻り値用）
    prog.push(Instruction::Duplicate);
    // 整数として出力
    prog.push(Instruction::OutputNumber);
    Ok(prog)
}

fn generate_builtin_putc(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    prog.append(generate_expression(ctx, &args[0])?);
    prog.push(Instruction::Duplicate);
    prog.push(Instruction::OutputChar);
    Ok(prog)
}

fn generate_builtin_geti(
    _ctx: &mut CodeGenContext,
    _args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    prog.push(Instruction::Push(WsNumber(heap_layout::TEMP_PTR)));
    prog.push(Instruction::Duplicate);
    prog.push(Instruction::InputNumber);
    prog.push(Instruction::Retrieve);
    Ok(prog)
}

fn generate_builtin_getc(
    _ctx: &mut CodeGenContext,
    _args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    prog.push(Instruction::Push(WsNumber(heap_layout::TEMP_PTR)));
    prog.push(Instruction::Duplicate);
    prog.push(Instruction::InputChar);
    prog.push(Instruction::Retrieve);
    Ok(prog)
}
```

### 引数チェック

引数の個数チェックを入れるかは任意。semantic_analyzer 側で既にチェックされている可能性がある。
最小限の実装では省略してよいが、防御的に以下のチェックを入れることを推奨:

- `__puti`、`__putc`: `args.len() == 1` でなければエラー
- `__geti`、`__getc`: `args.len() == 0` でなければエラー

### `TEMP_PTR` の競合について

`__geti()` と `__getc()` は一時領域 `TEMP_PTR`（アドレス 4）を使用する。
`TEMP_PTR` は `memory.rs` に定義済みで、他に現在使用箇所がないため競合しない。

ただし、将来的に `__geti() + __geti()` のような式では、先に評価した `__geti()` の結果はスタックに積まれているため、後の `__geti()` が `TEMP_PTR` を上書きしても問題ない（先の結果は既にスタック上にある）。

## 既存テストとの対応

`tests/compile_test.rs` に以下のテストケースが存在する（すべて `#[ignore]` 付き、wsc 必要）:

| テスト | 内容 | 使用する関数 |
|--------|------|-------------|
| `test_compile_and_run_puti` | `__puti(42)` → "42" | `__puti` |
| `test_compile_and_run_putc` | `__putc(65)` → "A" | `__putc` |
| `test_compile_and_run_arithmetic` | `__puti(1 + 2 * 3)` → "7" | `__puti` |
| `test_compile_and_run_variable` | 変数経由 `__puti(x)` → "123" | `__puti` |
| `test_compile_and_run_geti` | 入力 "10\n" → `__puti(x * 2)` → "20" | `__geti`, `__puti` |

## 備考

- `__trace`、`__assert`、`__assert_not` はヒープ経由の間接呼び出し方式で実装予定（`whitespace-runtime.md` に記載）。I/O 関数とは異なるアプローチ。
- `__getiv`、`__getcv`（アドレス指定入力）は docs/spec.md に記載がないため、今回のスコープ外とする。

---

## 実装結果 (2026-02-07)

### 実装完了

`src/compiler_ws/expression.rs` に以下の4つのビルトイン関数を実装しました：

- `generate_builtin_puti()` - `__puti(x)` 整数出力
- `generate_builtin_putc()` - `__putc(x)` 文字出力
- `generate_builtin_geti()` - `__geti()` 整数入力
- `generate_builtin_getc()` - `__getc()` 文字入力

`generate_function_call()` を修正し、関数名によるマッチングで各ビルトイン関数の実装を呼び出すようにしました。

### テスト結果

1. **コンパイルテスト**: 全てパス (8 passed; 0 failed; 5 ignored)
2. **生成コード検証**: ニーモニック形式での出力を確認し、設計通りのWhitespace命令が生成されることを確認
   - `__puti(42)` → `push 42; dup; printi; discard`
   - `__putc(65)` → `push 65; dup; printc; discard`
   - `__geti()` → `push 4; dup; readi; retrieve`
   - `__getc()` → `push 4; dup; readc; retrieve`
3. **実行テスト（wsc使用）**: 全て成功 ✅
   - `test_compile_and_run_puti`: `__puti(42)` → "42" 出力
   - `test_compile_and_run_putc`: `__putc(65)` → "A" 出力
   - `test_compile_and_run_arithmetic`: `__puti(1+2*3)` → "7" 出力
   - `test_compile_and_run_variable`: 変数経由の出力 → "123" 出力
   - `test_compile_and_run_geti`: 入力"10"→`x*2`出力 → "20" 出力
   - 追加検証: `__getc()` + `__putc()` でエコー動作を確認

### 変更ファイル

- `src/compiler_ws/expression.rs`: 約100行追加（4つの関数 + `generate_function_call` の修正）

### 今後の課題

- ユーザー定義関数の実装（現在は `UndefinedFunction` エラーを返す）
