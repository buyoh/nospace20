# compiler_ws/expression.rs リファクタリング設計

## 現状

[src/compiler_ws/expression.rs](../../../src/compiler_ws/expression.rs) は 1020 行で、24 個の関数が定義されている。
ファイル分割よりも、コード重複の解消によるリファクタリングが効果的。

## 重複パターンの分析

### 重複 1: Global/Local アドレス分岐 — 6 箇所

`match var_info.scope { VarScope::Global => ..., VarScope::Local => ... }` パターンが 6 箇所で重複。
核心は **ローカル変数のアドレス計算** `Push(offset) → Push(LHB) → Retrieve → Add` が散在していること。

| 関数 | 行 | Global パターン | Local パターン |
|------|-----|----------------|----------------|
| `generate_variable_address` | L104–117 | `Push(GLOBAL_PTR+offset)` | `Push(offset), Push(LHB), Retrieve, Add` |
| `generate_array_element_address` | L135–156 | `Push(base), expr, Add` | `Push(offset), expr, Add, Push(LHB), Retrieve, Add` |
| `generate_store_variable` | L472–496 | addr → Store → Retrieve | addr → Store → addr再計算 → Retrieve |
| `generate_store_array` | L508–545 | addr → Store → addr再計算 → Retrieve | addr → Store → addr再計算 → Retrieve |
| `generate_store_variable_void` | L933–949 | addr → Store | addr → Store |
| `generate_store_array_void` | L960–985 | addr → Store | addr → Store |

#### 改善案

Store 系関数は `generate_variable_address` / `generate_array_element_address` を内部で共用すればアドレス計算部分が不要になる。

```rust
// Store 系は既存のアドレス生成関数に委譲
fn generate_store_variable_impl(
    ctx: &mut CodeGenContext,
    var_ref: &IdentifierRef,
    value_expr: &LocatedExecExpression,
    emit_retrieve: bool,  // value版: true, void版: false
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    // アドレス計算は generate_variable_address に委譲
    prog.append(generate_variable_address(ctx, var_ref));
    // 値生成
    prog.append(generate_expression(ctx, value_expr)?);
    // Store
    prog.push(Instruction::Store);
    if emit_retrieve {
        // 値を返す場合、アドレスを再計算して Retrieve
        prog.append(generate_variable_address(ctx, var_ref));
        prog.push(Instruction::Retrieve);
    }
    Ok(prog)
}
```

### 重複 2: Store/Retrieve void バリアント — 2 ペア

| 値版 | void 版 | 差異 |
|------|---------|------|
| `generate_store_variable` (34行) | `generate_store_variable_void` (26行) | 末尾の Retrieve 有無 |
| `generate_store_array` (46行) | `generate_store_array_void` (34行) | 末尾の Retrieve 有無 |

#### 改善案

`emit_retrieve: bool` パラメータで統合:

```rust
// 4関数 → 2関数 に統合
fn generate_store_variable_impl(ctx, var_ref, value_expr, emit_retrieve: bool) -> ...
fn generate_store_array_impl(ctx, var_ref, index_expr, value_expr, emit_retrieve: bool) -> ...
```

呼び出し側（`generate_expression` と `generate_expression_as_statement`）が `emit_retrieve` を切り替え。

**削減見込み**: ~50 行

### 重複 3: 比較演算子 6 種 — 各約 13 行 × 6

L265–344 の 6 つの比較演算子 (`==`, `!=`, `<`, `<=`, `>`, `>=`) は全て同じ構造:

```
label1 = new_label; label2 = new_label;
eval(operand_a); eval(operand_b);
Sub;
JumpIfZero/JumpIfNegative(label1);
Push(false_val); Jump(label2);
Label(label1); Push(true_val);
Label(label2);
```

差異:
1. 使う Jump 命令 (`JumpIfZero` vs `JumpIfNegative`)
2. オペランドの順序 (`left, right` vs `right, left`)
3. true/false 値の配置

#### 改善案: データ駆動テーブル

```rust
struct ComparisonSpec {
    /// 最初のオペランド（left か right か）
    first_operand: Operand,
    /// 2番目のオペランド
    second_operand: Operand,
    /// 使用する条件ジャンプ
    jump_kind: JumpKind,
    /// ジャンプ先が true か false か
    jump_is_true: bool,
}

enum Operand { Left, Right }
enum JumpKind { Zero, Negative }

fn comparison_spec(op: &Operator2) -> ComparisonSpec {
    match op {
        Operator2::Equal       => ComparisonSpec { first_operand: Left, second_operand: Right, jump_kind: Zero, jump_is_true: true },
        Operator2::NotEqual    => ComparisonSpec { first_operand: Left, second_operand: Right, jump_kind: Zero, jump_is_true: false },
        Operator2::Less        => ComparisonSpec { first_operand: Left, second_operand: Right, jump_kind: Negative, jump_is_true: true },
        Operator2::LessEqual   => ComparisonSpec { first_operand: Right, second_operand: Left, jump_kind: Negative, jump_is_true: false },
        Operator2::Greater     => ComparisonSpec { first_operand: Right, second_operand: Left, jump_kind: Negative, jump_is_true: true },
        Operator2::GreaterEqual=> ComparisonSpec { first_operand: Left, second_operand: Right, jump_kind: Negative, jump_is_true: false },
        _ => unreachable!(),
    }
}

fn generate_comparison(
    ctx: &mut CodeGenContext,
    spec: &ComparisonSpec,
    left: &LocatedExecExpression,
    right: &LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    let label_jump = ctx.new_label();
    let label_end = ctx.new_label();

    // オペランドの順序に従って評価
    let (first, second) = match spec.first_operand {
        Operand::Left => (left, right),
        Operand::Right => (right, left),
    };
    prog.append(generate_expression(ctx, first)?);
    prog.append(generate_expression(ctx, second)?);
    prog.push(Instruction::Sub);

    // 条件ジャンプ
    match spec.jump_kind {
        JumpKind::Zero => prog.push(Instruction::JumpIfZero(label_jump)),
        JumpKind::Negative => prog.push(Instruction::JumpIfNegative(label_jump)),
    }

    // ジャンプしなかった場合の値
    prog.push(Instruction::Push(if spec.jump_is_true { 0 } else { 1 }));
    prog.push(Instruction::Jump(label_end));

    // ジャンプした場合の値
    prog.push(Instruction::Label(label_jump));
    prog.push(Instruction::Push(if spec.jump_is_true { 1 } else { 0 }));
    prog.push(Instruction::Label(label_end));

    Ok(prog)
}
```

**削減見込み**: 80行 → 約 35行 (~45 行削減)

## ファイル分割の検討

重複解消だけで約 130 行削減され、1020 → ~890 行になる。まだ大きいが、各関数が論理的に一体のコード生成パイプラインを形成しているため、無理にファイルを分割するメリットは少ない。

分割する場合の候補:

| 分割先 | 内容 | 行数 |
|--------|------|------|
| `expression_builtin.rs` | 組み込み関数コード生成 (puti/putc/geti/getc/debug/alloc/free) | ~200 行 |
| `expression.rs` (残り) | 式コード生成本体 | ~690 行 |

組み込み関数コード生成は `generate_function_call` からのみ呼ばれ、独立性が高い。

## 推奨実行順序

1. **Store/Retrieve void 統合** — 最も安全で効果が大きい（~50 行削減）
2. **比較演算子のデータ駆動化** — 構造的な改善（~45 行削減）
3. **アドレス計算の共用化** — Store 系関数のさらなる簡略化（~35 行削減）
4. **組み込み関数の分離** — 任意（ファイル分割）

## テストへの影響

- 生成される Whitespace コードが同一であることを `cargo test` で検証
- ピープホール最適化のテストも含めて全テスト通過を確認
- 命令列が変わらないリファクタリングのため、テスト修正は不要

## リスク

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| 生成コードの微妙な差異 | 中 | アドレス計算の共用化で命令順序が変わる可能性あり。既存テストで検証 |
| パフォーマンスへの影響 | 低 | コンパイラ側の効率であり、生成コードの実行効率には影響しない |
