# 代入文の値破棄最適化 (`discard-assign-value`)

## 概要

代入式 `x = expr` の結果値（式としての値）が使用されない場合、代入後の値再取得を省略する。代入は文として使われることが圧倒的に多いため、多くのケースで Retrieve 命令を削減できる。

## 問題

現在のコード生成（`generate_store_variable`）では、代入は常に式としての値をスタックに残す：

```rust
// グローバル変数への代入
prog.push(Instruction::Push(WsNumber(addr)));
prog.append(generate_expression(ctx, value_expr)?);
prog.push(Instruction::Store);
// 代入式の値として value を残す
prog.push(Instruction::Push(WsNumber(addr)));   // ← 不要な場合がある
prog.push(Instruction::Retrieve);                // ← 不要な場合がある
```

そして式文のコード生成で、直後に値が破棄される：

```rust
ExecStatement::Expression(expr) => {
    let mut prog = expression::generate_expression(ctx, expr)?;
    prog.push(Instruction::Discard);  // ← 上で Push + Retrieve した値を即破棄
    Ok(prog)
}
```

つまり `x = 5;` のような単純な代入文で：
- グローバル: Push + Retrieve + Discard = **3命令が無駄**
- ローカル: Push + Push + Retrieve + Add + Retrieve + Discard = **6命令が無駄**

配列代入 `arr[i] = v;` でも同様に、値再取得部分が無駄になる。

## 設計

### 方式: void コンテキスト導入

式のコード生成時に「値が使用されるか（value context）」または「値が不要か（void context）」を伝搬する。

#### 中間表現レベルの変更

`ExecStatement::Expression` が代入式を含む場合、代入式を**文レベルの代入**として扱う新しい `ExecStatement` バリアントを追加するか、コード生成に void コンテキストフラグを渡す。

**方式 A: コード生成に void フラグを渡す**（推奨）

```rust
// expression.rs
pub fn generate_expression(
    ctx: &mut CodeGenContext,
    located_expr: &LocatedExecExpression,
    void_context: bool,  // 値が不要な場合 true
) -> Result<WsProgram, CompileError>
```

代入のコード生成で `void_context == true` なら値再取得をスキップ：

```rust
Operator2::Assign if void_context => {
    // Store のみ、Retrieve はスキップ
    match &left.expression {
        ExecExpression::Variable(var_ref) => {
            prog.append(generate_store_variable_void(ctx, var_ref, right)?);
        }
        // ...
    }
}
```

**方式 B: 中間表現に `AssignStatement` を追加**

`ExecStatement::AssignStatement(lhs, rhs)` を追加し、意味解析で代入式の式文を自動変換する。
→ 中間表現の変更が大きいため、方式 A を推奨。

### 変更対象ファイル

| ファイル | 変更内容 |
|---|---|
| `src/compiler_ws/expression.rs` | `generate_expression` に `void_context` パラメータ追加、代入時の値再取得スキップ |
| `src/compiler_ws/statement.rs` | `ExecStatement::Expression` で `void_context: true` を渡す |

### 命令削減量（推定）

| パターン | 変数種別 | 削減命令数 |
|---|---|---|
| `x = expr;` | グローバル | 3命令 (Push+Retrieve+Discard) |
| `x = expr;` | ローカル | 6命令 (Push+Push+Retrieve+Add+Retrieve+Discard) |
| `arr[i] = expr;` | グローバル | 4命令 |
| `arr[i] = expr;` | ローカル | 7命令 |
| `*ptr = expr;` | — | 3命令 |

代入文はプログラム中で最も頻繁な操作の一つであるため、累計削減量は大きい。

### 注意点

- 連鎖代入 `x = y = 5;` では外側の代入のみ void context とし、内側は value context のまま
- `void_context` を `generate_expression` 全体に渡すが、代入以外の式では無視してよい
- while 本体のブロック値（必ず Discard される）にも拡張可能

## テスト

- 既存テスト全通過の確認
- 代入文・連鎖代入・配列代入の動作確認
- プロファイルによる命令数削減の計測
