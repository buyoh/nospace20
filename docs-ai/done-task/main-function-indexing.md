# Scope.identifier_map 縮小とmain関数インデックス化の実装完了

実装日: 2026-02-11

## 概要

[symbol-table-design.md](../task/symbol-table-design.md) のステップ4「Scope.identifier_map の縮小」を実装しました。これにより、main 関数の取得が文字列検索からインデックスベースでのアクセスに変更され、ランタイムでの文字列マッチングが不要になりました。

## 実装内容

### 1. Scope に main_function_index フィールドを追加

[src/semantic_analyzer/scope.rs](../../src/semantic_analyzer/scope.rs) に新しいフィールドを追加:

```rust
pub struct Scope {
    // ... 既存のフィールド ...

    /// main 関数のインデックス（存在する場合）
    /// Phase 6: 関数名による検索を排除し、インデックスベースでアクセス
    pub main_function_index: Option<usize>,

    // ...
}
```

### 2. ScopeBuilder で main 関数インデックスを解決

`ScopeBuilder::build()` メソッドで、`function_names` の中から "main" を探してインデックスを設定:

```rust
// Phase 6: main 関数のインデックスを解決
let main_function_index = function_names
    .iter()
    .position(|name| name == "main");

Scope {
    // ...
    main_function_index,
    // ...
}
```

### 3. interpreter を更新

#### interpret_all の変更

[src/interpreter/mod.rs](../../src/interpreter/mod.rs) で、main 関数の取得をインデックスベースに変更:

```rust
pub fn interpret_all(env: &mut Environment, scope: &Scope) -> Option<i64> {
    interpret_global(env, scope);
    // Phase 6: main_function_index を使用してインデックスベースでアクセス
    if let Some(main_idx) = scope.main_function_index {
        let func = &scope.functions[main_idx];
        let mut e = LocalEnvironment::new_func(env, scope, func, &Vec::<i64>::new());
        let res = e.interpret_statements(&func.block.statements);
        if let Flow::Return(x) = res {
            Some(x)
        } else {
            None
        }
    } else {
        eprintln!("error: function 'main' not found");
        None
    }
}
```

#### initialize_function_statics の変更

`get_function` を使わずに、直接 `functions` をイテレート:

```rust
fn initialize_function_statics(env: &mut Environment, scope: &Scope) {
    // Phase 6: インデックスベースで関数にアクセス
    for (func_idx, func) in scope.functions.iter().enumerate() {
        let has_static = func.block.scope.variables.iter().any(|v| v.is_static);
        if !has_static {
            continue;
        }

        // ... 初期化処理 ...

        // Phase 6: 関数名ではなくインデックスを使用
        let func_name = &scope.function_names[func_idx];
        env.function_static_storage.insert(func_name.clone(), storage);
    }
}
```

### 4. compiler_ws を更新

[src/compiler_ws/statement.rs](../../src/compiler_ws/statement.rs) で同様に変更:

```rust
pub fn generate_scope(
    ctx: &mut CodeGenContext,
    scope: &Scope,
) -> Result<WsProgram, CompileError> {
    // ...

    // Phase 6: main_function_index を使用してインデックスベースでアクセス
    if let Some(main_idx) = scope.main_function_index {
        let main_func = &scope.functions[main_idx];
        prog.append(generate_function_definition(ctx, "main", main_func)?);
    }

    Ok(prog)
}
```

### 5. semantic analyzer の Scope 初期化を更新

[src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs) で、Scope を初期化する際に `main_function_index` を追加:

- 関数プレースホルダー作成時: `main_function_index: None`
- 一時スコープ作成時: `main_function_index: None`

## テスト

### 新規追加テスト

2つのテストケースを追加:

1. [main_function_index_001.ns](../../resources/tests/passes/main_idx/main_function_index_001.ns) - 単純な main 関数
2. [main_function_index_002.ns](../../resources/tests/passes/main_idx/main_function_index_002.ns) - 他の関数と共に main 関数

### テスト結果

すべてのテストがパスしました:
- `cargo test --lib`: 158 テスト全てパス ✓
- `cargo test --test code_test`: 109 テスト全てパス ✓
- `cargo test --test compile_test`: 1 テスト全てパス ✓
- 新規テスト2件も正しく動作 ✓

## パフォーマンス向上

この変更により、以下の改善が得られました:

**変更前**:
- main 関数取得: `get_function("main")` → `identifier_map` での文字列検索
- static 変数初期化: 各関数について `get_function(name)` → 文字列検索の繰り返し

**変更後**:
- main 関数取得: `scope.functions[main_idx]` → O(1) の配列アクセス
- static 変数初期化: `functions.iter().enumerate()` → 直接イテレート

## 残存する文字列使用

以下の箇所では依然として文字列を使用していますが、これらは次のステップで対応予定:

1. **`Scope.identifier_map`**: 依然として存在するが、main 関数以外での使用
2. **`Scope.function_names`**: static 変数初期化時にストレージのキーとして使用
3. **`function_static_storage`**: 文字列キーで永続ストレージを管理（ステップ5で対応予定）
4. **`interpret_func`**: 名前で関数を呼び出す API（テスト用に残存）

## 次のステップ

[symbol-table-design.md](../task/symbol-table-design.md) のステップ5へ:
- **function_static_storage のインデックスキー化**: 関数名ではなく関数インデックスをキーとして使用

## 影響を受けるファイル

- `src/semantic_analyzer/scope.rs`: `Scope` に `main_function_index` 追加、`ScopeBuilder::build()` で解決
- `src/semantic_analyzer/mod.rs`: Scope 初期化時に `main_function_index` を設定
- `src/interpreter/mod.rs`: `interpret_all` と `initialize_function_statics` をインデックスベースに変更
- `src/compiler_ws/statement.rs`: `generate_scope` をインデックスベースに変更
- `resources/tests/passes/main_idx/`: 新規テスト2件追加

## 関連ドキュメント

- [symbol-table-design.md](../task/symbol-table-design.md): 全体設計
- [builtin-function-indexing.md](./builtin-function-indexing.md): ステップ3（完了）
- [variable-identifier-to-slot-index.md](./variable-identifier-to-slot-index.md): ステップ2（完了）
- [function-args-identifier-resolution-completed.md](./function-args-identifier-resolution-completed.md): ステップ1（完了）
