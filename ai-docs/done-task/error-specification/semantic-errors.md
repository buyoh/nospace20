# 意味解析エラー (Semantic Analysis Errors)

## 概要

意味解析フェーズでは、構文的に正しいコードの意味的妥当性を検証する。このフェーズで検出されるエラーは、変数・関数のスコープ、名前の重複、不正な構文の使用などに関するものである。

**実装場所**: `src/semantic_analyzer/mod.rs`  
**エラー型**: `CodeParseError`

## エラー一覧

### 1. 未定義の変数参照

**エラーメッセージ**:
```
undefined variable: {name}
```

**発生条件**: スコープ内に定義されていない変数を参照

**テストケース**: `resources/tests/fails/compile/scope_undefined_001.ns`

**ソースコード**: `src/semantic_analyzer/mod.rs:159`

```rust
.ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", v))])?;
```

**例**:
```nospace
func: main() {
  __clog(v);  # エラー: v が定義されていない
}
```

---

### 2. 名前の重複

**エラーメッセージ**:
```
semantic error: the name '{name}' is already used
```

**発生条件**: 同じスコープ内で同じ名前を複数回定義

**テストケース**: `resources/tests/fails/compile/scope_duplicate_001.ns`

**ソースコード**: `src/semantic_analyzer/mod.rs:385-389`

```rust
if self.identifier_map.contains_key(name) {
    return Err(vec![code_parse_error!(format!(
        "semantic error: the name '{}' is already used",
        name
    ))]);
}
```

**例**:
```nospace
func: main() {
  let: x;
  let: x;  # エラー: x は既に定義されている
  return: 0;
}
```

---

### 3. ネストした関数宣言

**エラーメッセージ**:
```
semantic error: nested function declaration is not supported
```

**発生条件**: 関数の中で別の関数を定義しようとする

**ソースコード**: `src/semantic_analyzer/mod.rs:457-460`

```rust
return Err(vec![code_parse_error!(
    format!("semantic error: nested function declaration is not supported")
)]);
```

**例**:
```nospace
func: outer() {
  func: inner() {  # エラー: ネストした関数宣言は非サポート
    return: 1;
  }
  return: 2;
}
```

---

### 4. 関数外での return 文

**エラーメッセージ**:
```
semantic error: return statement outside of function
```

**発生条件**: 関数の外で return 文を使用

**ソースコード**: `src/semantic_analyzer/mod.rs:547-550`

```rust
return Err(vec![code_parse_error!(
    format!("semantic error: return statement outside of function")
)]);
```

**例**:
```nospace
let: x;
x = 10;
return: x;  # エラー: 関数外で return を使用

func: main() {
  return: 0;
}
```

---

### 5. 関数外での continue 文

**エラーメッセージ**:
```
semantic error: continue statement outside of function
```

**発生条件**: 関数の外（またはループの外）で continue 文を使用

**ソースコード**: `src/semantic_analyzer/mod.rs:564-567`

```rust
return Err(vec![code_parse_error!(
    format!("semantic error: continue statement outside of function")
)]);
```

**注**: 現在の実装では「関数外」のみをチェックしており、「ループ外」のチェックは行われていない可能性がある

**例**:
```nospace
let: x;
continue;  # エラー: 関数外で continue を使用

func: main() {
  return: 0;
}
```

---

### 6. 関数外での break 文

**エラーメッセージ**:
```
semantic error: break statement outside of function
```

**発生条件**: 関数の外（またはループの外）で break 文を使用

**ソースコード**: `src/semantic_analyzer/mod.rs:573-576`

```rust
return Err(vec![code_parse_error!(
    format!("semantic error: break statement outside of function")
)]);
```

**注**: 現在の実装では「関数外」のみをチェックしており、「ループ外」のチェックは行われていない可能性がある

**例**:
```nospace
let: x;
break;  # エラー: 関数外で break を使用

func: main() {
  return: 0;
}
```

---

## テストケースの網羅性

現在のテストケース：

| テストケース | パス | カバーしているエラー |
|------------|------|-------------------|
| `scope_undefined_001.ns` | `fails/compile/` | 未定義の変数参照 |
| `scope_duplicate_001.ns` | `fails/compile/` | 名前の重複 |
| `scope_out_of_scope_001.ns` | `fails/compile/` | スコープ外の変数参照 |

### 不足しているテストケース

- [ ] ネストした関数宣言
- [ ] 関数外での return 文
- [ ] 関数外での continue 文
- [ ] 関数外での break 文
- [ ] ループ外での continue 文（関数内だがループ外）
- [ ] ループ外での break 文（関数内だがループ外）
- [ ] 未定義の関数呼び出し

## 実装上の注意点

### 1. break/continue のループ外チェック

現在の実装では、以下のケースが検出されない可能性がある：

```nospace
func: main() {
  let: x;
  x = 10;
  break;  # ループ外だがエラーにならない？
  return: 0;
}
```

意味解析器が「関数内かどうか」のみをチェックしており、「ループ内かどうか」を追跡していない可能性がある。この点は実装を確認する必要がある。

### 2. エラーメッセージの一貫性

意味解析エラーのメッセージには `"semantic error: "` プレフィックスが付くものと付かないものがある：

- **プレフィックスあり**: 名前の重複、ネストした関数宣言、return/break/continue の不正な使用
- **プレフィックスなし**: 未定義の変数参照

一貫性のため、全ての意味解析エラーに同じプレフィックスを付けることを検討すべき。

### 3. エラーメッセージの多言語対応

現在、エラーメッセージは全て英語で記述されている。将来的に多言語対応を行う場合、エラーコードを導入し、メッセージを外部化することを検討すべき。

## 改善提案

### 1. より詳細なスコープ情報

変数が定義されている位置を示すことで、ユーザーの理解を助ける：

**現状**:
```
semantic error: the name 'x' is already used
```

**改善案**:
```
semantic error: the name 'x' is already used
  first defined at line 3, column 8
  redefined at line 5, column 8
```

### 2. ループコンテキストの追跡

break/continue がループ内でのみ使用されることを保証するため、意味解析器にループコンテキストの追跡機能を追加：

```rust
struct ScopeBuilder {
    // 既存のフィールド...
    in_loop: bool,  // 現在のスコープがループ内かどうか
}
```

### 3. 未定義関数の検出

現在、未定義の関数呼び出しの検出はコンパイルフェーズで行われている可能性がある。意味解析フェーズで検出することで、より早期にエラーを報告できる。
