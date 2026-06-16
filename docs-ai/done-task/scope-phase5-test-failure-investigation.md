# Phase 5 テスト失敗調査: test_error_nested_function_declaration

## テスト失敗の概要

Phase 5 のネスト関数フラット化実装後、以下のテストが失敗:

```
---- semantic_analyzer::tests::test_error_nested_function_declaration stdout ----
thread 'semantic_analyzer::tests::test_error_nested_function_declaration' panicked at src/semantic_analyzer/tests.rs:105:5:
assertion failed: result.is_err()
```

## 失敗したテスト

- **テスト名**: `test_error_nested_function_declaration`
- **場所**: `src/semantic_analyzer/tests.rs:82-117`
- **テスト内容**: ネスト関数宣言がエラーになることを確認

```rust
#[test]
fn test_error_nested_function_declaration() {
    // func: outer() { func: inner() {} }
    let inner_func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "inner".to_string(),
            vec![],
            vec![], // empty body
        ),
        location: SourceLocation::new(100, 120),
    };

    let outer_func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "outer".to_string(),
            vec![],
            vec![inner_func],
        ),
        location: SourceLocation::new(80, 130),
    };

    let statements = vec![outer_func];
    let result = analyze(&statements);
    assert!(result.is_err()); // ← ここで失敗

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code_pointer, Some(100));
    assert!(errors[0]
        .message
        .contains("nested function declaration is not supported"));
}
```

## 失敗原因

### 根本的な理由

このテストは、**Phase 5 以前の仕様**に基づいており、ネスト関数がサポートされていないことを確認するものだった。

Phase 5 の実装により:
- ネスト関数が正式にサポートされた
- 全関数がルートスコープにフラット化される設計が採用された
- ネスト関数宣言は**エラーではなく正常な動作**となった

### Phase 5 の設計目標との関係

[phase5-stack-overflow-investigation.md](phase5-stack-overflow-investigation.md) では、以下の設計方針が採用された:

> **方針B（採用）**: 全関数をルートスコープにフラットに格納。名前解決はスコープごとに行うが、インデックスはグローバル。

この設計により、ネスト関数は文法的に許可され、実行時に正しく動作する。

## 対処方針

### 選択肢1: テストを削除 (推奨)

Phase 5 でネスト関数がサポートされたため、このテストは時代遅れになった。テストを削除するのが適切。

### 選択肢2: テストを更新

ネスト関数が**成功する**ことを確認するテストに変更:

```rust
#[test]
fn test_nested_function_declaration() {
    // func: outer() { func: inner() {} }
    let inner_func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "inner".to_string(),
            vec![],
            vec![],
        ),
        location: SourceLocation::new(100, 120),
    };

    let outer_func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "outer".to_string(),
            vec![],
            vec![inner_func],
        ),
        location: SourceLocation::new(80, 130),
    };

    let statements = vec![outer_func];
    let result = analyze(&statements);
    assert!(result.is_ok()); // ネスト関数は成功する
}
```

ただし、統合テスト (`resources/tests/passes/scope/scope_nested_func_001.ns`) で既にカバーされているため、削除で十分。

## 結論

**このテスト失敗は、Phase 5 の実装が正しく動作している証拠**である。

- Phase 5 の目標: ネスト関数のスタックオーバーフローを解決 ✓
- 副作用: ネスト関数が文法的にサポートされる ✓
- 既存のテストとの矛盾: 意図的な仕様変更

**推奨アクション**: `test_error_nested_function_declaration` を削除する。

## 関連ファイル

- [src/semantic_analyzer/tests.rs](../../../src/semantic_analyzer/tests.rs) - 失敗したテストの場所
- [phase5-stack-overflow-investigation.md](phase5-stack-overflow-investigation.md) - Phase 5 の設計ドキュメント
- [resources/tests/passes/scope/scope_nested_func_001.ns](../../../resources/tests/passes/scope/scope_nested_func_001.ns) - ネスト関数の統合テスト
