# 最適化パスフレームワーク設計

## 概要

意味解析後の中間表現 (`Scope`) に対して、複数の最適化パスを順次適用するフレームワークを設計する。各パスはプラグイン的に有効化・無効化でき、依存関係を考慮した実行順序を持つ。

## モジュール構成

### 現在の実装 (フレームワーク完了時点)

```
src/
  optimizer/
    mod.rs               # パス管理・実行エントリポイント ✅
    noop_test_pass.rs    # フレームワーク動作検証用ダミーパス ✅
    tests.rs             # ユニットテスト (5件) ✅
```

### 最終構成 (全パス実装後)

```
src/
  optimizer/
    mod.rs               # パス管理・実行エントリポイント
    noop_test_pass.rs    # フレームワーク動作検証用ダミーパス
    tests.rs             # ユニットテスト
    constant_folding.rs   # 定数畳み込み
    condition_opt.rs      # if/while 条件式最適化
    geti_opt.rs           # __geti/__getc 最適化
    dead_code.rs          # 未使用コード削除
```

## ExecExpression の拡張

最適化パスが生成する新しい式を、**最小限のバリアント追加**で実現する。最適化の種類ごとに専用バリアントを追加するのではなく、既存バリアントの拡張と汎用的な内部バリアントで対応する。

### 設計方針

| 課題 | 旧設計（非採用） | 新設計 |
|---|---|---|
| 条件式最適化 (if) | `IfZero`, `IfNegative` 追加 | 既存 `If` に `ConditionMode` フィールド追加 |
| 条件式最適化 (while) | `WhileNotZero`, `WhileNegative` 追加 | 既存 `While` に `ConditionMode` フィールド追加 |
| 入力最適化 | `InternalGetiv`, `InternalGetcv` 追加 | 汎用 `InternalBuiltinFunction` バリアント追加 |
| **合計** | **6 新バリアント** | **1 新バリアント + 2 既存バリアント修正** |

### 追加する型

#### ConditionMode

```rust
/// 条件式の評価モード
///
/// If/While の条件式がどのように true/false を判定するかを指定する。
/// 意味解析では常に NonZero が使用される。最適化パスが Zero/Negative に変換する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConditionMode {
    /// cond != 0 → true（既存動作、意味解析が生成）
    NonZero,
    /// cond == 0 → true（Whitespace: JumpIfZero を直接使用）
    Zero,
    /// cond < 0 → true（Whitespace: JumpIfNegative を直接使用）
    Negative,
}
```

#### InternalBuiltinFunctionKind

```rust
/// 最適化パスで生成される内部組み込み関数の種類
///
/// 各バリアントは必要なデータを自身に保持する。
/// 意味解析では生成されず、最適化パスでのみ生成される。
pub(crate) enum InternalBuiltinFunctionKind {
    /// 標準入力から整数を読み、変数に直接格納（TEMP_PTR 経由を排除）
    Getiv(IdentifierRef),
    /// 標準入力から文字を読み、変数に直接格納
    Getcv(IdentifierRef),
}
```

### ExecExpression の変更

```rust
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<LocatedExecExpression>),
    Operation2(Operator2, Box<LocatedExecExpression>, Box<LocatedExecExpression>),
    // ConditionMode を第1引数に追加（既存は全て ConditionMode::NonZero）
    If(ConditionMode, Box<LocatedExecExpression>, Block, Block),
    While(ConditionMode, Box<LocatedExecExpression>, Block),
    Block(Block),
    BuiltinFunction(BuiltinFunctionKind, Vec<Box<LocatedExecExpression>>),
    UserFunction(IdentifierRef, Vec<Box<LocatedExecExpression>>),
    Factor(i64),
    Variable(IdentifierRef),
    ArrayAccess(IdentifierRef, Box<LocatedExecExpression>, usize),
    /// 最適化パスで生成される内部組み込み関数
    InternalBuiltinFunction(InternalBuiltinFunctionKind),
}
```

### セマンティクス詳細

#### If の ConditionMode 別動作

| ConditionMode | then ブロック実行条件 | else ブロック実行条件 | WS 命令 |
|---|---|---|---|
| `NonZero` | cond != 0 | cond == 0 | 既存（COMPARATOR 経由） |
| `Zero` | cond == 0 | cond != 0 | `JumpIfZero` 直接 |
| `Negative` | cond < 0 | cond >= 0 | `JumpIfNegative` 直接 |

#### While の ConditionMode 別動作

| ConditionMode | ループ継続条件 | ループ終了条件 | WS 命令 |
|---|---|---|---|
| `NonZero` | cond != 0 | cond == 0 | 既存（COMPARATOR 経由） |
| `Zero` | cond == 0 | cond != 0 | `JumpIfZero` で継続 |
| `Negative` | cond < 0 | cond >= 0 | `JumpIfNegative` で継続 |

> **`While(NonZero, ...)` → `While(Zero, ...)`の変換例**: `while: expr != 0 { body }` の条件式 `expr != 0` は、`COMPARATOR_ZERO` を呼んだ結果が nonzero かどうかで判定する（2段階）。`While(Zero, expr, body)` に変換すると、`JumpIfZero` で `expr == 0` のとき直接ループ終了できるため、比較サブルーチンが不要になる。なお、`While(NonZero, ...)` と `While(Zero, ...)` はループ継続/終了条件が逆になる点に注意。最適化パス側で条件式の意味を適切に変換する必要がある。

### 型推論への影響

```rust
impl ExecExpression {
    pub(crate) fn infer_type(&self, func_return_types: &[ValueType]) -> ValueType {
        match self {
            // ConditionMode は型推論に影響しない
            ExecExpression::While(_, _, _) => ValueType::Void,
            ExecExpression::If(_, _, then_block, else_block) => {
                infer_block_type(then_block, func_return_types)
                    .merge(infer_block_type(else_block, func_return_types))
            }
            ExecExpression::InternalBuiltinFunction(kind) => match kind {
                InternalBuiltinFunctionKind::Getiv(_) => ValueType::Int,
                InternalBuiltinFunctionKind::Getcv(_) => ValueType::Int,
            },
            // ... 既存のまま ...
        }
    }
}
```

### リファクタリング影響範囲

`If`/`While` に `ConditionMode` フィールドを追加するため、以下の箇所を機械的に修正する必要がある:

| ファイル | 変更内容 |
|---|---|
| `semantic_analyzer/types.rs` | enum 定義変更、`infer_type` の match パターン更新 |
| `semantic_analyzer/mod.rs` | `ExecExpression::If(...)` / `While(...)` 構築時に `ConditionMode::NonZero` を第1引数に追加 (2箇所) |
| `interpreter/exec.rs` | `interpret_expression` の match パターン更新 (If, While)、`interpret_if`/`interpret_while` に ConditionMode 対応追加、`InternalBuiltinFunction` ハンドラ追加 |
| `compiler_ws/expression.rs` | `generate_expression` の match パターン更新 (If, While)、`generate_if_expression`/`generate_while_expression` に ConditionMode 対応追加、`InternalBuiltinFunction` コード生成追加 |
| `optimizer/noop_test_pass.rs` | `ExecExpression` パターンに影響なし（直接 If/While を使用していない） |

意味解析が生成する `If`/`While` は常に `ConditionMode::NonZero` を指定するため、**既存の動作に変更はない**。

## パス管理

### OptimizationOptions

#### 現在の実装

```rust
pub struct OptimizationOptions {
    /// テスト用パス: マジックナンバー変数を追加する（フレームワーク検証用）
    pub noop_test_pass: bool,
}
```

`none()` / `all()` / `any_enabled()` / `Default` を持つ。
`all()` は現時点では `noop_test_pass: false` (テスト用パスは含めない)。

#### 最終設計 (各パス実装時にフィールドを追加)

```rust
/// 最適化オプション
pub struct OptimizationOptions {
    /// テスト用パス（フレームワーク検証用、本番では無効）
    pub noop_test_pass: bool,
    /// 定数畳み込み
    pub constant_folding: bool,
    /// if/while 条件式最適化 (Whitespace 向け)
    pub condition_optimization: bool,
    /// __geti/__getc 最適化 (Whitespace 向け)
    pub geti_optimization: bool,
    /// 未使用関数の削除
    pub dead_function_elimination: bool,
}
```

### デフォルトプリセット

```rust
impl OptimizationOptions {
    /// 最適化なし
    pub fn none() -> Self { ... }

    /// Whitespace コンパイル向けの全最適化
    pub fn whitespace_all() -> Self {
        Self {
            constant_folding: true,
            condition_optimization: true,
            geti_optimization: true,
            dead_function_elimination: true,
        }
    }
}
```

### パス実行順序

#### 現在の実装

```rust
pub fn optimize(scope: &mut Scope, options: &OptimizationOptions) {
    if !options.any_enabled() {
        return;
    }
    // テスト用パス
    if options.noop_test_pass {
        noop_test_pass::apply(scope);
    }
}
```

#### 最終設計

```rust
pub fn optimize(scope: &mut Scope, options: &OptimizationOptions) {
    // 1. 定数畳み込み（他の最適化のパターンマッチを容易にする）
    if options.constant_folding {
        constant_folding::optimize(scope);
    }

    // 2. 条件式最適化（定数畳み込み後に実行: 定数条件 if の除去が先に行われるため）
    if options.condition_optimization {
        condition_opt::optimize(scope);
    }

    // 3. __geti/__getc 最適化
    if options.geti_optimization {
        geti_opt::optimize(scope);
    }

    // 4. 未使用関数削除（他の最適化でコードが変わった後に実行）
    if options.dead_function_elimination {
        dead_code::optimize(scope);
    }
}
```

### 実行順序の根拠

1. **定数畳み込み → 条件式最適化**: `if: 3 == 0 { ... }` のような式は定数畳み込みで `if: 0 { ... }` になり、さらに条件が定数なのでブロックスコープに変換できる
2. **定数畳み込み → 未使用関数削除**: 定数畳み込みの結果、呼び出しが除去される関数が出る可能性がある
3. **条件式最適化 → 未使用関数削除**: 同上

## パイプラインへの統合 ✅ 実装済み

### lib.rs の変更

```rust
mod optimizer;  // 新規モジュール

// compile_with_options から呼び出す、または明示的な API を追加
pub fn optimize(scope: &mut Scope, options: &optimizer::OptimizationOptions) {
    optimizer::optimize(scope, options);
}
```

### CLI の変更 (compile_property.rs) ✅ 実装済み

```rust
pub struct CompileProperty {
    // 既存フィールド...

    /// 最適化レベル (0 = なし, 1 = 全最適化)
    pub optimization_level: u8,
}
```

CLI オプション:

```
--opt=0       最適化なし（デフォルト、既存動作との互換性）
--opt=1       全最適化
```

初期実装では `0` と `1` のみ。将来的に個別パスの制御も可能。

## Interpreter への影響

`If`/`While` の match パターンに `ConditionMode` を追加し、モードに応じた条件判定を行う。`InternalBuiltinFunction` のハンドラも追加。

```rust
// If: ConditionMode に応じた分岐
ExecExpression::If(mode, cond, then_block, else_block) => {
    let val = eval(cond);
    let condition = match mode {
        ConditionMode::NonZero => val != 0,
        ConditionMode::Zero => val == 0,
        ConditionMode::Negative => val < 0,
    };
    if condition { eval_block(then_block) } else { eval_block(else_block) }
}

// While: ConditionMode に応じたループ判定
ExecExpression::While(mode, cond, block) => {
    loop {
        let val = eval(cond);
        let condition = match mode {
            ConditionMode::NonZero => val != 0,
            ConditionMode::Zero => val == 0,
            ConditionMode::Negative => val < 0,
        };
        if !condition { break; }
        eval_block(block);
    }
}

// InternalBuiltinFunction
ExecExpression::InternalBuiltinFunction(kind) => match kind {
    InternalBuiltinFunctionKind::Getiv(var_ref) => {
        let value = read_integer_from_stdin();
        set_variable(var_ref, value);
        ExpressionFlow::Value(value)
    }
    InternalBuiltinFunctionKind::Getcv(var_ref) => {
        let value = read_char_from_stdin();
        set_variable(var_ref, value);
        ExpressionFlow::Value(value)
    }
}
```

## Compiler WS への影響

新バリアントのコード生成を追加する。詳細は各パスのドキュメントを参照。

## Scope の変更可能性

Scope は不変（`&Scope`）として各バックエンドに渡されているため、最適化パスは `&mut Scope` を受け取る新しいフェーズとして動作する。パス適用後は通常通り `&Scope` として消費される。

ただし、Scope の構造（`variables`, `functions` 等）は `Vec` ベースのインデックス参照を多用しているため、要素の削除には注意が必要:

- **関数削除**: `IdentifierRef.local_index` が無効になるリスク → 削除ではなくマーキング（空の関数に置換）が安全
- **変数削除**: `variable_count` やスロットインデックスに影響 → 初期段階では未実装

## テスト戦略

- 各最適化パスの Unit テスト: パターンマッチと変換の正しさを検証
- 既存テストケースの回帰テスト: 最適化有無で実行結果が変わらないことを確認
- プロファイラによる効果測定: 命令数・ステップ数の比較
