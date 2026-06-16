# interpreter モジュール ファイル分割 - 完了報告

## 実施日

2026年2月7日

## 実施内容

`src/interpreter/mod.rs`（622行）を責務ごとに4ファイルに分割しました。

### 分割結果

```
interpreter/
├── mod.rs           # モジュール宣言、re-exports、公開 API (~55行)
├── types.rs         # Flow, ExpressionFlow, try_expr!, bool_to_int (~32行)
├── environment.rs   # EnvironmentConfig, EnvironmentMetrics, Environment (~199行)
└── exec.rs          # LocalEnvironment + 全実行ロジック (~382行)
```

### ファイル別詳細

#### `mod.rs` (55行)
- サブモジュール宣言（`mod types;`, `mod environment;`, `mod exec;`）
- re-exports（`pub use environment::{Environment, EnvironmentConfig, EnvironmentMetrics};`）
- 公開 API 関数: `interpret_func()`, `interpret()`

#### `types.rs` (32行)
- `enum Flow` — ブロックの評価結果（Proceed, Return, Continue, Break）
- `enum ExpressionFlow` — 式の評価結果（Value, Jump）
- `macro_rules! try_expr` — ExpressionFlow の早期リターンマクロ
- `fn bool_to_int()` — ユーティリティ関数

#### `environment.rs` (199行)
- `pub struct EnvironmentConfig` — 実行制限設定
- `pub struct EnvironmentMetrics` — 実行メトリクス
- `pub struct Environment` — グローバル実行環境（I/O、トレース、設定、グローバル変数）
- 各 struct の impl ブロック

#### `exec.rs` (382行)
- `struct LocalEnvironment` — ローカル実行環境
- `impl LocalEnvironment` — 全実行メソッド:
  - 変数操作: `new_func`, `enter_block`, `leave_block`, `get_variable`, `set_variable`
  - 組み込み関数: `interpret_call_function`
  - ユーザー関数: `interpret_call_user_function`
  - 制御フロー: `interpret_while`, `interpret_if`
  - 演算: `interpret_operation1`, `interpret_operation2`
  - 式評価: `interpret_expression`
  - 文実行: `interpret_statement`, `interpret_statements`, `interpret_statements_with_value`

## 可視性設計

| 要素 | 可視性 | 理由 |
|------|--------|------|
| `Flow` | `pub(super)` | mod.rs から参照が必要 |
| `ExpressionFlow` | `pub(super)` | exec.rs 内部で使用 |
| `try_expr!` | `pub(super)` | exec.rs で使用 |
| `bool_to_int` | `pub(super)` | exec.rs で使用 |
| `EnvironmentConfig` | `pub` | lib.rs で re-export |
| `EnvironmentMetrics` | `pub` | lib.rs で re-export 可能 |
| `Environment` | `pub` | lib.rs で re-export |
| `LocalEnvironment` | `pub(super)` | mod.rs の公開 API から構築 |
| `LocalEnvironment::new_func` | `pub(super)` | mod.rs から呼び出し |
| `LocalEnvironment::interpret_statement` | `pub(super)` | mod.rs から呼び出し |
| `LocalEnvironment::interpret_statements` | `pub(super)` | mod.rs から呼び出し |

## テスト結果

- `cargo build`: 成功（警告は既存の未使用コードに関するもの）
- `cargo test`: 全72テストが成功

```
test result: ok. 72 passed; 0 failed; 14 ignored; 0 measured; 0 filtered out
```

## 外部インターフェースへの影響

**なし。** `lib.rs` からの re-export パスは `interpreter::Environment` 等のまま変わりません。

## 利点

1. **責務の明確化**: 型定義、環境管理、実行ロジックが物理的に分離
2. **可読性の向上**: 各ファイルが200行前後に収まり、理解しやすい
3. **将来の拡張への準備**:
   - ユニットテスト追加が容易（[unit-test-interpreter.md](../task/unit-test-interpreter.md) 参照）
   - suspendable interpreter の実装が容易（[suspendable-interpreter/](../task/suspendable-interpreter/) 参照）
   - Phase 2 でさらなる分割（`builtins.rs`, `operations.rs`, `session.rs`）が可能

## 今後の展開（Phase 2 以降）

1. `exec.rs` からさらに分割:
   - `builtins.rs` — 組み込み関数
   - `operations.rs` — 演算処理
2. テスタビリティ向上:
   - メソッドを独立関数に抽出し `pub(crate)` で公開
   - ユニットテストの追加
3. suspendable interpreter のための `session.rs` 追加

## 関連ドキュメント

- タスク仕様: [docs-ai/task/interpreter-split.md](interpreter-split.md)
- ユニットテスト計画: [docs-ai/task/unit-test-interpreter.md](../task/unit-test-interpreter.md)
- Suspendable interpreter: [docs-ai/task/suspendable-interpreter/](../task/suspendable-interpreter/)
