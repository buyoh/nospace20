# interpreter ユニットテスト追加

## 概要

interpreter モジュールにユニットテストを追加するためのタスク。
現状ユニットテストがなく、追加には設計変更（サブモジュール分割）が必要。

## 背景

[unit-test-analysis.md](../done-task/unit-test-analysis.md) の分析結果より分離。

## 現状の課題

1. **個別関数のテスト困難**: `interpret_expression()`, `interpret_statement()` 等は private
2. **Environment のモック困難**: stdin/stdout のモックは `new_with_buffers()` で可能だが、テスト用ヘルパーがない
3. `LocalEnvironment` は private struct
4. 公開インターフェースは `interpret_func()` のみ

## 改善タスク

### Phase 1: サブモジュール分割

- [ ] **T1-1**: interpreter を builtins.rs, operations.rs に分割
  - `builtins.rs`: `__trace`, `__assert`, `__puti`, `__putc`, `__geti`, `__getc`
  - `operations.rs`: `bool_to_int`, 二項演算の評価処理
  - 各関数を `pub(crate)` で公開

### Phase 2: テストヘルパー整備

- [ ] **T2-1**: ExecExpression/ExecStatement を手動構築するためのビルダー追加
  ```rust
  #[cfg(test)]
  mod test_helpers {
      use super::*;
      
      pub fn make_exec_number(value: i64) -> ExecExpression {
          ExecExpression::Number(value)
      }
      pub fn make_exec_binary(op: &str, left: ExecExpression, right: ExecExpression) -> ExecExpression {
          ExecExpression::BinaryOperator(op.to_string(), Box::new(left), Box::new(right))
      }
      pub fn make_exec_call(func_id: usize, args: Vec<ExecExpression>) -> ExecExpression {
          ExecExpression::FunctionCall(func_id, args)
      }
      // Scope, Function の構築ヘルパーも追加
  }
  ```
- [ ] **T2-2**: Environment 構築ヘルパー追加（stdin/stdout モック含む）
- [ ] **T2-3**: 外部ファイル（JSON/YAML）によるテストケース定義の検討
  - 複雑な Scope/Function を記述する場合、可読性のため外部ファイル化を検討
  - `resources/tests/unit/interpreter/` にテストデータを配置

### Phase 3: ユニットテスト追加

- [ ] **T3-1**: interpreter のユニットテスト追加（10件程度）
  - 組み込み関数（ExecExpression を手動構築）
  - 演算子
  - 制御フロー

**注意**: ユニットテストでは前段のモジュール（token_parser / tree_parser / semantic_analyzer）に依存せず、
`Scope` や `ExecExpression` を直接構築すること。文字列からパースするテストは結合テストとして別途実施する。

## 推奨されるモジュール構造

```
interpreter/
├── mod.rs           # 公開 API (interpret_func)
├── environment.rs   # Environment の定義と実装
├── builtins.rs      # 組み込み関数 (__trace, __puti, __geti, __getc, __putc)
├── operations.rs    # 演算処理 (bool_to_int, 二項演算等)
└── test.rs          # ユニットテスト
```

**分割のポイント**:
- `builtins.rs`: 各組み込み関数を `pub(crate)` で公開し、個別テスト可能に
- `operations.rs`: 純粋関数として分離し、副作用なしでテスト可能に

## 可視性設計例

```rust
// builtins.rs
pub(crate) fn builtin_trace(env: &mut Environment, key: i64) -> i64 {
    if let Some(v) = env.traced.get_mut(&key) {
        *v += 1;
    } else {
        env.traced.insert(key, 1);
    }
    0
}

// test.rs
#[cfg(test)]
mod test {
    use super::builtins::builtin_trace;
    use super::Environment;

    #[test]
    fn test_builtin_trace() {
        let mut env = Environment::new();
        builtin_trace(&mut env, 42);
        assert_eq!(env.traced.get(&42), Some(&1));
        builtin_trace(&mut env, 42);
        assert_eq!(env.traced.get(&42), Some(&2));
    }
}
```

## 推奨テストケース

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
| test_bool_to_int | true/false 変換 | 1/0 |

## 既存のテスト用関数

`lib.rs` に既にテスト用関数が存在:

```rust
pub fn interpret_func_testing(scope: &Scope, func_name: &str) -> BTreeMap<i64, i64>
pub fn interpret_func_with_io(scope: &Scope, func_name: &str, stdin: &str) -> (BTreeMap<i64, i64>, String)
```

これらを活用してテストを拡充できる。

## 優先度

**中** - semantic_analyzer と同等、サブモジュール分割により大幅改善可能

## 参考

- 元の分析: [unit-test-analysis.md](../done-task/unit-test-analysis.md)
