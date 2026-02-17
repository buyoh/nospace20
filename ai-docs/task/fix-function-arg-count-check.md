# 関数呼び出し引数数チェックの実装

## 概要

関数呼び出し時に引数の数が不足（または過剰）していてもコンパイルエラーにも実行時エラーにもならないバグを修正する。意味解析フェーズでのコンパイルエラーとして検出する。

## 現状の問題

### ユーザー定義関数

- **引数不足**: エラーなし。不足した引数は暗黙的に `0` として扱われる
  - `exec.rs` L238-243 のループが早く終了するだけで、残りのパラメータ変数は初期値 `0` のまま
- **引数過剰**: エラーなし。余剰引数は評価されるが捨てられる
  - `if i < func.arg_indices.len()` ガードで無視される

### 組み込み関数

- **引数不足**: `args.first().unwrap()` による実行時 panic（ユーザーフレンドリーでないエラー）
- **引数過剰**: エラーなし。余剰引数は無視される

## 修正方針

**意味解析（semantic_analyzer）フェーズでコンパイルエラーとして検出する。**

- 実行前に全ての引数数不整合を検出できる
- エラーメッセージで「期待される引数数」と「実際の引数数」を表示できる

## 設計

### 変更ファイル

1. `src/semantic_analyzer/scope.rs` — `FunctionIndex` に引数数を追加、`ScopeResolver` にメソッド追加
2. `src/semantic_analyzer/mod.rs` — 関数呼び出し時のチェックロジック追加
3. テストケース追加（`resources/tests/fails/compile/`）

### Step 1: `FunctionIndex` に引数数を追加（scope.rs）

```rust
// 変更前
pub(super) struct FunctionIndex(pub usize);

// 変更後
pub(super) struct FunctionIndex(pub usize, pub usize); // (global_index, arg_count)
```

`FunctionIndex.1` に宣言時のパラメータ数を保持する。

### Step 2: `ScopeResolver` に引数数取得メソッドを追加（scope.rs）

```rust
/// 関数の期待される引数数を取得する
pub fn get_function_arg_count(&self, name: &str) -> Option<usize> {
    for scope_info in self.scope_stack.iter().rev() {
        if let Some(Identifier::Function(info)) = scope_info.func_map.get(name) {
            return Some(info.1);
        }
    }
    None
}
```

### Step 3: パス1a で引数数を登録（mod.rs）

`analyze_internal_with_parent` のパス1a（関数宣言スキャン）で、`FunctionDeclaration` の引数リストを利用する：

```rust
// 変更前
Statement::FunctionDeclaration(name, _, _) => {
    // ...
    scope.add_identifier(
        name,
        Identifier::Function(FunctionIndex(global_idx)),
    )?;
}

// 変更後
Statement::FunctionDeclaration(name, args, _) => {
    // ...
    scope.add_identifier(
        name,
        Identifier::Function(FunctionIndex(global_idx, args.len())),
    )?;
}
```

### Step 4: 関数呼び出し時の引数数チェック（mod.rs）

`convert_to_exec_expression_with_resolver` の `Expression::Function` 処理に追加：

**ユーザー定義関数:**

```rust
let func_ref = parent_resolver.resolve_function(f).ok_or_else(|| {
    vec![code_parse_error!(format!("undefined function: {}", f))]
})?;

// 引数数チェック
let expected_count = parent_resolver.get_function_arg_count(f)
    .expect("function should be resolvable");
if args.len() != expected_count {
    return Err(vec![code_parse_error!(format!(
        "function '{}' expects {} argument(s), but {} were provided",
        f, expected_count, args.len()
    ))]);
}
```

**組み込み関数:**

`BuiltinFunctionKind` ごとに期待される引数数を定義し、チェックする：

```rust
if let Some(kind) = builtin_kind {
    // 組み込み関数の引数数チェック
    let expected = match kind {
        BuiltinFunctionKind::Puti => 1,
        BuiltinFunctionKind::Putc => 1,
        BuiltinFunctionKind::Geti => 0,
        BuiltinFunctionKind::Getc => 0,
        BuiltinFunctionKind::Clog => 1,
        BuiltinFunctionKind::Assert => 1,
        BuiltinFunctionKind::AssertNot => 1,
        BuiltinFunctionKind::Trace => 1,
    };
    if args.len() != expected {
        return Err(vec![code_parse_error!(format!(
            "builtin function '{}' expects {} argument(s), but {} were provided",
            f, expected, args.len()
        ))]);
    }
    Ok(Box::new(ExecExpression::BuiltinFunction(kind, args)))
}
```

### Step 5: `resolve_function` の修正（scope.rs）

`FunctionIndex` のフィールド参照を修正する。`info.0` は引き続きグローバルインデックスとして使用。

```rust
pub fn resolve_function(&self, name: &str) -> Option<IdentifierRef> {
    for (_depth, scope_info) in self.scope_stack.iter().rev().enumerate() {
        if let Some(Identifier::Function(info)) = scope_info.func_map.get(name) {
            return Some(IdentifierRef {
                scope_depth: 0,
                local_index: info.0,  // FunctionIndex.0 = global_index（変更なし）
                is_global: true,
            });
        }
    }
    None
}
```

### Step 6: テストケースの追加

以下のテストケースを `resources/tests/fails/compile/` に追加し、`test-manifest.yaml` に登録する。

#### テスト1: ユーザー定義関数の引数不足

ファイル: `func_arg_too_few_001.ns`
```
func:add(a,b){return:a+b;}
func:main(){__trace(add(1));return:0;}
```

チェック: `func_arg_too_few_001.check.json`
```json
{
  "type": "compile_error",
  "contains": ["add"]
}
```

#### テスト2: ユーザー定義関数の引数過剰

ファイル: `func_arg_too_many_001.ns`
```
func:inc(a){return:a+1;}
func:main(){__trace(inc(1,2));return:0;}
```

チェック: `func_arg_too_many_001.check.json`
```json
{
  "type": "compile_error",
  "contains": ["inc"]
}
```

#### テスト3: 組み込み関数の引数不足

ファイル: `builtin_arg_too_few_001.ns`
```
func:main(){__puti();return:0;}
```

チェック: `builtin_arg_too_few_001.check.json`
```json
{
  "type": "compile_error",
  "contains": ["__puti"]
}
```

#### テスト4: 組み込み関数の引数過剰

ファイル: `builtin_arg_too_many_001.ns`
```
func:main(){__clog(1,2);return:0;}
```

チェック: `builtin_arg_too_many_001.check.json`
```json
{
  "type": "compile_error",
  "contains": ["__clog"]
}
```

#### テスト5: 引数なし関数の正常呼び出し確認（回帰テスト）

これは既存の passes テストで十分カバーされている（`__geti()`, `__getc()` 等の呼び出し）。新規追加は不要。

## 影響範囲

- **意味解析器（semantic_analyzer）**: `scope.rs` と `mod.rs` の変更
- **インタプリタ（interpreter）**: 変更不要（意味解析で弾かれるため到達しない）
- **コンパイラ（compiler_ws）**: 変更不要（意味解析で弾かれるため到達しない）
- **既存テスト**: 引数数が正しいテストは影響なし。引数数が間違ったテストが存在した場合は修正が必要

## エラーメッセージ形式

```
semantic error: function 'add' expects 2 argument(s), but 1 were provided
semantic error: builtin function '__puti' expects 1 argument(s), but 0 were provided
```

## ステータス

- [x] Step 1: `FunctionIndex` に引数数を追加
- [x] Step 2: `ScopeResolver` にメソッド追加
- [x] Step 3: パス1a で引数数を登録
- [x] Step 4: 関数呼び出し時の引数数チェック
- [x] Step 5: `resolve_function` 動作確認（修正不要のはず）
- [x] Step 6: テストケース追加
- [x] 既存テストの確認・全テスト通過

## 実装完了

2026年2月17日に実装完了。

### 実装内容

1. `src/semantic_analyzer/scope.rs`:
   - `FunctionIndex` を `(usize, usize)` に変更し、引数数も保持
   - `ScopeResolver::get_function_arg_count()` メソッドを追加

2. `src/semantic_analyzer/mod.rs`:
   - パス1aで関数宣言時に引数数を `FunctionIndex` に登録
   - 関数呼び出し時にユーザー定義関数と組み込み関数の両方で引数数チェックを実装

3. テストケース追加:
   - `func_arg_too_few_001.ns` / `func_arg_too_many_001.ns` (ユーザー定義関数)
   - `builtin_arg_too_few_001.ns` / `builtin_arg_too_many_001.ns` (組み込み関数)

### テスト結果

- 新規追加した4つのテストすべてが成功
- 既存テストのうち3つ (`test_control_flow_if_expr_value_001_ws_self`, `test_scope_block_expr_nested_001_ws_self`, `test_scope_block_expr_value_001_ws_self`) は私の変更前から失敗しており、既存のバグであることを確認
