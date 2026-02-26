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

最適化パスが生成する**内部専用**の ExecExpression バリアントを追加する。これらは意味解析では生成されず、最適化パスでのみ生成される。

### 追加バリアント

```rust
pub(crate) enum ExecExpression {
    // --- 既存のバリアント ---
    Operation1(Operator1, Box<LocatedExecExpression>),
    Operation2(Operator2, Box<LocatedExecExpression>, Box<LocatedExecExpression>),
    If(Box<LocatedExecExpression>, Block, Block),
    While(Box<LocatedExecExpression>, Block),
    Block(Block),
    BuiltinFunction(BuiltinFunctionKind, Vec<Box<LocatedExecExpression>>),
    UserFunction(IdentifierRef, Vec<Box<LocatedExecExpression>>),
    Factor(i64),
    Variable(IdentifierRef),
    ArrayAccess(IdentifierRef, Box<LocatedExecExpression>, usize),

    // --- 最適化パスで追加されるバリアント (条件式最適化) ---
    /// if (cond == 0) { then } else { else }
    /// JumpIfZero を直接使用。比較サブルーチン呼び出しを排除。
    IfZero(Box<LocatedExecExpression>, Block, Block),
    /// if (cond < 0) { then } else { else }
    /// JumpIfNegative を直接使用。
    IfNegative(Box<LocatedExecExpression>, Block, Block),
    /// while (cond == 0) { body }
    WhileNotZero(Box<LocatedExecExpression>, Block),
    /// while (cond < 0) { body }
    WhileNegative(Box<LocatedExecExpression>, Block),

    // --- 最適化パスで追加されるバリアント (__geti/__getc 最適化) ---
    /// InputNumber を変数アドレスに直接書き込み（一時領域を経由しない）
    InternalGetiv(IdentifierRef),
    /// InputChar を変数アドレスに直接書き込み
    InternalGetcv(IdentifierRef),
}
```

### セマンティクス詳細

| バリアント | 条件 | then ブロック実行 | else ブロック実行 |
|---|---|---|---|
| `If(cond, then, else)` | cond != 0 → then | cond == 0 → else | 既存 |
| `IfZero(expr, then, else)` | expr == 0 → then | expr != 0 → else | **新規** |
| `IfNegative(expr, then, else)` | expr < 0 → then | expr >= 0 → else | **新規** |

| バリアント | ループ継続条件 | ループ終了条件 |
|---|---|---|
| `While(cond, body)` | cond != 0 | cond == 0 | 既存 |
| `WhileNotZero(expr, body)` | expr != 0 | expr == 0 | **新規** |
| `WhileNegative(expr, body)` | expr < 0 | expr >= 0 | **新規** |

> **`WhileNotZero` について**: 名前は「ゼロでない間ループ」。既存の `While` と同じループ継続/終了条件だが、条件式が「比較結果(0/1)」ではなく「生の値」であることを示す。Whitespace コンパイラは `JumpIfZero` を直接使用でき、比較サブルーチン呼び出しが不要になる。`While(expr != 0, body)` から変換され、`expr != 0` の比較コード生成をスキップする。

### 型推論への影響

新バリアントの型推論:

```rust
ExecExpression::IfZero(_, then_block, else_block) => {
    // If と同じ: 両ブロックの型をマージ
    infer_block_type(then_block, func_return_types)
        .merge(infer_block_type(else_block, func_return_types))
}
ExecExpression::IfNegative(_, then_block, else_block) => {
    // 同上
    infer_block_type(then_block, func_return_types)
        .merge(infer_block_type(else_block, func_return_types))
}
ExecExpression::WhileNotZero(_, _) => ValueType::Void,  // while と同じ
ExecExpression::WhileNegative(_, _) => ValueType::Void,
ExecExpression::InternalGetiv(_) => ValueType::Int,
ExecExpression::InternalGetcv(_) => ValueType::Int,
```

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

新しい ExecExpression バリアントは、Interpreter でもハンドリングする必要がある（コンパイルエラーを避けるため）。ただし、Interpreter 向けの最適化は条件式最適化・geti 最適化を含まないため、通常は到達しない。

```rust
// interpreter での処理（安全のため実装）
ExecExpression::IfZero(cond, then_block, else_block) => {
    let val = eval(cond);
    if val == 0 { eval_block(then_block) } else { eval_block(else_block) }
}
ExecExpression::IfNegative(cond, then_block, else_block) => {
    let val = eval(cond);
    if val < 0 { eval_block(then_block) } else { eval_block(else_block) }
}
// ...
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
