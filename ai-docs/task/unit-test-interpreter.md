# interpreter ユニットテスト追加

## 概要

interpreter モジュールにユニットテストを追加するためのタスク。

## 背景

[unit-test-analysis.md](../done-task/unit-test-analysis.md) の分析結果より分離。

## 実施済み

### サブモジュール分割 (2026-02-07完了)

[interpreter-split.md](../done-task/interpreter-split.md) により、以下の構造に分割済み:

```
interpreter/
├── mod.rs           # 公開 API (interpret_func, interpret_all 等)
├── environment.rs   # Environment の定義と実装
├── exec.rs          # 実行ロジック (LocalEnvironment, interpret_expression 等)
└── types.rs         # 型定義 (Flow, ExpressionFlow, bool_to_int)
```

当初提案の `builtins.rs` / `operations.rs` 分割は採用されず、上記の責務ベース分割が実施された。

### 既存ユニットテスト (exec.rs 内、4件)

`exec.rs` 末尾に以下のテストが存在（パーサ経由の統合テスト形式）:

1. `test_resolve_address_local_variables` - ローカル変数のアドレス解決
2. `test_get_set_by_address` - アドレスによる値の取得・設定
3. `test_ref_and_deref_integration` - 参照・デリファレンスの統合テスト
4. `test_deref_assign_integration` - デリファレンス代入の統合テスト

テストヘルパーとして `create_test_env()`, `parse_and_analyze()` が存在。

## 残タスク

### 組み込み関数・演算のテスト追加

- [ ] **T1**: 組み込み関数のテスト追加（builtins 分離不要、パーサ経由で可）
  - `__trace`, `__assert`, `__puti`, `__putc`, `__geti`, `__getc`
- [ ] **T2**: 二項演算子のテスト追加
  - 算術: `+`, `-`, `*`, `/`, `%`
  - 比較: `==`, `!=`, `<`, `>`, `<=`, `>=`
  - 論理: `&&`, `||`
- [ ] **T3**: 制御フローのテスト追加
  - if/else, while, return, break, continue

### 推奨テストケース

| テスト名 | 内容 | 期待結果 |
|---------|------|----------|
| test_builtin_trace | `__trace(1)` を2回 | traced に {1: 2} |
| test_builtin_assert_pass | `__assert(1)` | 何も起きない |
| test_builtin_assert_fail | `__assert(0)` | panic |
| test_builtin_puti | `__puti(42)` | stdout に "42" |
| test_builtin_putc | `__putc(65)` | stdout に "A" |
| test_builtin_geti | stdin "42" | 42 を返す |
| test_builtin_getc | stdin "A" | 65 を返す |
| test_binary_add | `1 + 2` | 3 |
| test_binary_mul | `3 * 4` | 12 |
| test_binary_logical_and | `1 && 0` | 0 |
| test_binary_logical_or | `0 || 1` | 1 |

## 既存のテスト用関数

`lib.rs` に既にテスト用関数が存在:

```rust
pub fn interpret_func_testing(scope: &Scope, func_name: &str) -> BTreeMap<i64, i64>
pub fn interpret_func_with_io(scope: &Scope, func_name: &str, stdin: &str) -> (BTreeMap<i64, i64>, String)
```

これらを活用してテストを拡充できる。

## 優先度

**低** - 既にテストの土台が存在し、結合テスト（code_test.rs, compile_test.rs）でカバーされている部分も多い。追加は nice-to-have。

## 参考

- 元の分析: [unit-test-analysis.md](../done-task/unit-test-analysis.md)
- サブモジュール分割: [interpreter-split.md](../done-task/interpreter-split.md)
