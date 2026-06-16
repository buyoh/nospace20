# interpreter モジュール ファイル分割

## 概要

`src/interpreter/mod.rs`（622行）を責務ごとに複数ファイルに分割する。
将来の拡張（ユニットテスト追加、suspendable interpreter）の土台となるリファクタリング。

## 背景

- `mod.rs` に型定義、環境管理、実行ロジック、組み込み関数、公開 API がすべて詰め込まれている
- [unit-test-interpreter.md](unit-test-interpreter.md) で分割の必要性が既に指摘されている
- [suspendable-interpreter/](suspendable-interpreter/) で `session.rs` の追加が予定されている
- 他モジュール（`tree_parser` 等）は既にサブファイルに分割済み

## 現状の構造

| 行範囲 | 内容 | 行数（概算） |
|---------|------|-------------|
| 1-16 | use 文、モジュールドキュメント | 16 |
| 18-37 | `Flow`, `ExpressionFlow`, `try_expr!` マクロ | 20 |
| 41-87 | `EnvironmentConfig`, `EnvironmentMetrics` | 47 |
| 89-224 | `Environment` | 136 |
| 226-583 | `LocalEnvironment` + 全 impl メソッド | 358 |
| 586-622 | 公開 API (`interpret_func`, `interpret`) | 37 |

## 分割設計

### Phase 1: 基本分割（本タスク）

```
interpreter/
├── mod.rs           # モジュール宣言、re-exports、公開 API
├── types.rs         # Flow, ExpressionFlow, try_expr!, bool_to_int
├── environment.rs   # EnvironmentConfig, EnvironmentMetrics, Environment
└── exec.rs          # LocalEnvironment + 全実行ロジック
```

#### `mod.rs`（~40行）

- サブモジュール宣言（`mod types;`, `mod environment;`, `mod exec;`）
- re-exports（`pub use environment::{Environment, EnvironmentConfig, EnvironmentMetrics};`）
- 公開 API 関数: `interpret_func()`, `interpret()`

#### `types.rs`（~30行）

- `enum Flow` — ブロックの評価結果（Proceed, Return, Continue, Break）
- `enum ExpressionFlow` — 式の評価結果（Value, Jump）
- `macro_rules! try_expr` — ExpressionFlow の早期リターンマクロ
- `fn bool_to_int()` — ユーティリティ

#### `environment.rs`（~180行）

- `pub struct EnvironmentConfig` — 実行制限設定
- `pub struct EnvironmentMetrics` — 実行メトリクス
- `pub struct Environment` — グローバル実行環境（I/O、トレース、設定、グローバル変数）
- 各 struct の `impl` ブロック

#### `exec.rs`（~360行）

- `struct LocalEnvironment` — ローカル実行環境
- `impl LocalEnvironment` — 全実行メソッド:
  - 変数操作: `new_func`, `enter_block`, `leave_block`, `get_variable`, `set_variable`
  - 組み込み関数: `interpret_call_function`
  - ユーザー関数: `interpret_call_user_function`
  - 制御フロー: `interpret_while`, `interpret_if`
  - 演算: `interpret_operation1`, `interpret_operation2`
  - 式評価: `interpret_expression`
  - 文実行: `interpret_statement`, `interpret_statements`, `interpret_statements_with_value`

### Phase 2: さらなる分割（将来・別タスク）

[unit-test-interpreter.md](unit-test-interpreter.md) および [suspendable-interpreter/](suspendable-interpreter/) と連携:

```
interpreter/
├── mod.rs
├── types.rs
├── environment.rs
├── exec.rs          # LocalEnvironment、式・文の実行
├── builtins.rs      # 組み込み関数（exec.rs から分離、pub(crate) 化）
├── operations.rs    # 演算処理（exec.rs から分離、pub(crate) 化）
├── session.rs       # InterpreterSession（suspendable interpreter 用）
└── test.rs          # ユニットテスト
```

Phase 2 は `exec.rs` 内のメソッドを独立関数に抽出し `pub(crate)` で公開することで、個別テストを可能にする。
本タスクでは Phase 2 の分割は行わない。

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

## 依存関係

```
mod.rs ──→ types.rs
  │  ──→ environment.rs
  │  ──→ exec.rs ──→ types.rs
  │                ──→ environment.rs
```

- `exec.rs` は `types.rs`（Flow, ExpressionFlow, try_expr!）と `environment.rs`（Environment）に依存
- `mod.rs` は `exec.rs`（LocalEnvironment）と `environment.rs`（Environment）に依存
- `types.rs` は他ファイルに依存しない

## 実装手順

### T1: types.rs の作成

1. `Flow`, `ExpressionFlow`, `try_expr!` マクロ, `bool_to_int` を `types.rs` に移動
2. `mod.rs` に `mod types;` を追加

### T2: environment.rs の作成

1. `EnvironmentConfig`, `EnvironmentMetrics`, `Environment` を `environment.rs` に移動
2. `mod.rs` に `mod environment;` と re-export を追加

### T3: exec.rs の作成

1. `LocalEnvironment` と全 impl メソッドを `exec.rs` に移動
2. `exec.rs` に必要な `use` 文を追加
3. `mod.rs` に `mod exec;` を追加

### T4: mod.rs の整理

1. `mod.rs` に残った公開 API 関数（`interpret`, `interpret_func`）を整理
2. 必要な `use` 文を調整

### T5: コンパイル・テスト確認

1. `cargo build` が通ることを確認
2. `cargo test` で既存テスト全件パスを確認

## 外部インターフェースの変更

**なし。** `lib.rs` からの re-export パスは `interpreter::Environment` 等のまま変わらない。

## 関連ドキュメント

- [unit-test-interpreter.md](unit-test-interpreter.md) — 分割後のユニットテスト追加計画
- [suspendable-interpreter/](suspendable-interpreter/) — 中断・再開可能なインタプリタの設計
- [architecture/modules.md](../architecture/modules.md) — モジュール詳細
