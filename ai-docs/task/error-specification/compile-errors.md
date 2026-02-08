# コンパイルエラー (Compile Errors)

## 概要

コンパイルフェーズでは、意味解析済みのコード（Scope）を Whitespace プログラムに変換する。このフェーズで検出されるエラーは、主にコード生成時に発生する問題である。

**実装場所**: `src/compiler_ws/mod.rs`  
**エラー型**: `CompileError` (enum)

## エラー一覧

### 1. main 関数が見つからない

**エラー種別**: `CompileError::MainNotFound`

**エラーメッセージ**:
```
main function not found
```

**発生条件**: プログラムに `main` 関数が定義されていない

**テストケース**: `resources/tests/fails/compile/no_main_001.ns`

**ソースコード**: `src/compiler_ws/mod.rs:41` (enum 定義), `src/compiler_ws/builtin.rs` (実際の検出箇所と推測)

**例**:
```nospace
func: foo() {
  return: 1;
}
# エラー: main 関数が存在しない
```

---

### 2. 未定義の変数

**エラー種別**: `CompileError::UndefinedVariable(String)`

**エラーメッセージ**:
```
Undefined variable: {name}
```

**発生条件**: コード生成中に未定義の変数を参照しようとした

**ソースコード**: `src/compiler_ws/mod.rs:38-39, 48`

```rust
pub enum CompileError {
    UndefinedVariable(String),
    // ...
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            // ...
        }
    }
}
```

**注**: 通常、このエラーは意味解析フェーズで検出されるべきであり、コンパイルフェーズでは発生しないはず。コンパイラの防御的プログラミングとして実装されている可能性がある。

---

### 3. 未定義の関数

**エラー種別**: `CompileError::UndefinedFunction(String)`

**エラーメッセージ**:
```
Undefined function: {name}
```

**発生条件**: コード生成中に未定義の関数を呼び出そうとした

**ソースコード**: `src/compiler_ws/mod.rs:39, 49`

```rust
UndefinedFunction(String),
// ...
CompileError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
```

**注**: 通常、このエラーは意味解析フェーズで検出されるべきであり、コンパイルフェーズでは発生しないはず。

---

### 4. 不正な操作

**エラー種別**: `CompileError::InvalidOperation(String)`

**エラーメッセージ**:
```
Invalid operation: {msg}
```

**発生条件**: コード生成中に不正な操作を検出（詳細はコンテキスト依存）

**ソースコード**: `src/compiler_ws/mod.rs:41, 51`

```rust
InvalidOperation(String),
// ...
CompileError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
```

**考えられる原因**:
- 未実装の演算子や命令
- サポートされていない式の組み合わせ
- コード生成ロジックの内部エラー

---

## CompileError の実装詳細

### エラー型の定義

```rust
#[derive(Debug)]
pub enum CompileError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}
```

### トレイト実装

**Display トレイト**:
```rust
impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            CompileError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            CompileError::MainNotFound => write!(f, "main function not found"),
            CompileError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
        }
    }
}
```

**Error トレイト**:
```rust
impl std::error::Error for CompileError {}
```

---

## テストケースの網羅性

現在のテストケース：

| テストケース | パス | カバーしているエラー |
|------------|------|-------------------|
| `no_main_001.ns` | `fails/compile/` | MainNotFound |
| `scope_undefined_001.ns` | `fails/compile/` | (意味解析でキャッチされる) |
| `scope_duplicate_001.ns` | `fails/compile/` | (意味解析でキャッチされる) |

### 不足しているテストケース

- [ ] UndefinedVariable（意味解析をすり抜けた場合）
- [ ] UndefinedFunction（意味解析をすり抜けた場合）
- [ ] InvalidOperation（具体的なケースを特定する必要あり）

---

## 検討事項

### 1. 意味解析との責任分界

現在、`UndefinedVariable` と `UndefinedFunction` は以下の2箇所で検出される可能性がある：

- **意味解析フェーズ**: `CodeParseError` として報告
- **コンパイルフェーズ**: `CompileError` として報告

**問題**:
- 責任の重複
- エラーメッセージの不一致（"undefined variable" vs "Undefined variable"）
- ユーザーから見てエラー報告が不安定に見える

**提案**:
意味解析フェーズで全ての変数・関数の存在チェックを行い、コンパイルフェーズでは検出しない（または `unreachable!()` として扱う）

### 2. InvalidOperation の詳細化

`InvalidOperation` は汎用的すぎるため、具体的なエラーケースごとに個別のバリアントを定義すべき：

**改善案**:
```rust
pub enum CompileError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    UnsupportedExpression(String),  // 未サポートの式
    UnsupportedStatement(String),   // 未サポートの文
    CodeGenError(String),           // 内部コード生成エラー
}
```

### 3. エラーコンテキストの追加

現在のエラーメッセージには位置情報が含まれていない。以下の情報を追加すべき：

```rust
pub enum CompileError {
    UndefinedVariable {
        name: String,
        location: Option<SourceLocation>,
    },
    // ...
}
```

---

## 実装調査が必要な項目

以下の点について、実際のコードを詳細に調査する必要がある：

1. **InvalidOperation が発生する具体的なケース**
   - `src/compiler_ws/expression/mod.rs` を調査
   - `src/compiler_ws/statement/mod.rs` を調査

2. **UndefinedVariable/UndefinedFunction の実際の使用箇所**
   - 意味解析で全てキャッチされているか
   - コンパイラでも実際に発生するケースがあるか

3. **MainNotFound の検出箇所**
   - `src/compiler_ws/builtin.rs` の `generate_footer` 関数と推測されるが、確認が必要

---

## 関連ファイル

- `src/compiler_ws/mod.rs` - CompileError の定義
- `src/compiler_ws/builtin.rs` - 組み込みルーチン生成（MainNotFound の検出？）
- `src/compiler_ws/context.rs` - コード生成コンテキスト
- `src/compiler_ws/expression/mod.rs` - 式のコード生成
- `src/compiler_ws/statement/mod.rs` - 文のコード生成
