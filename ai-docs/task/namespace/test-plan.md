# 名前空間: テスト計画

## Unit テスト

### Token Parser

| テスト | 内容 |
|--------|------|
| `test_namespace_keyword` | `namespace:` がキーワードとして認識されること |
| `test_dot_token` | `.` が `Token::Dot` として認識されること |
| `test_namespace_not_keyword_without_colon` | `namespace` がコロンなしの場合は識別子として認識されること |

### Tree Parser

| テスト | 内容 |
|--------|------|
| `test_namespace_empty` | `namespace: A {}` が正しくパースされること |
| `test_namespace_with_let` | `namespace: A { let: x; }` が正しくパースされること |
| `test_namespace_nested` | ネストした名前空間のパース |
| `test_qualified_variable` | `A.x` が修飾識別子としてパースされること |
| `test_qualified_function_call` | `A.f()` が修飾関数呼び出しとしてパースされること |
| `test_qualified_chain` | `A.B.x` が多段修飾識別子としてパースされること |

### Semantic Analyzer

| テスト | 内容 |
|--------|------|
| `test_namespace_variable_mangling` | 変数名がプレフィックス付きでマングルされること |
| `test_namespace_function_mangling` | 関数名がプレフィックス付きでマングルされること |
| `test_namespace_name_resolution` | 名前空間内での暗黙的名前解決 |
| `test_namespace_qualified_access` | ドット記法でのアクセス |
| `test_namespace_not_scope` | 名前空間がスコープを作成しないこと |
| `test_namespace_duplicate_error` | 同名名前空間の重複エラー |
| `test_namespace_name_collision` | 名前空間名と変数名の衝突エラー |

## Large テスト（`resources/tests/`）

### 成功テストケース (`passes/`)

| ファイル名 | 内容 |
|-----------|------|
| `namespace-basic.ns` | 基本的な名前空間と修飾アクセス |
| `namespace-nested.ns` | ネストした名前空間 |
| `namespace-function.ns` | 名前空間内の関数定義・呼び出し |
| `namespace-constexpr.ns` | 名前空間内の constexpr 定義 |
| `namespace-alias.ns` | 名前空間内の alias 定義 |
| `namespace-name-resolution.ns` | 名前空間内外の名前解決優先順位 |
| `namespace-static.ns` | 名前空間内の static 変数 |
| `namespace-with-scope.ns` | 名前空間とブロックスコープの組み合わせ |

### 失敗テストケース (`fails/semantic/`)

| ファイル名 | 内容 |
|-----------|------|
| `namespace-duplicate.ns` | 同名名前空間の重複定義 |
| `namespace-undefined-access.ns` | 未定義の名前空間への修飾アクセス |
| `namespace-name-collision.ns` | 名前空間名と変数名の衝突 |

## 動作確認例（`tmp/` での手動テスト）

```nospace
func: __main() {
  let: x(1);
  namespace: MySpace {
    let: x(2);
    namespace: MySpace2 {
      let: x(3);
    }
    __clog(x);           # 2 #
    __clog(MySpace2.x);  # 3 #
  }
  __clog(x);                  # 1 #
  __clog(MySpace.x);          # 2 #
  __clog(MySpace.MySpace2.x); # 3 #
  return: 0;
}
```

期待される出力: `2 3 1 2 3`（`__clog` は改行区切り）
