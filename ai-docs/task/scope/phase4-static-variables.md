# Phase 4: static 変数

## 目標

明示的な `static` 修飾子による変数宣言をサポートし、親の関数スコープを越えたアクセスを可能にする。

## 言語仕様 (spec.md セクション 7)

> - 親の関数スコープにはアクセスできない（変数の場合）
> - (未実装) static 変数は親の関数スコープにアクセス出来る。
> - 親の関数スコープで定義された関数にアクセス出来るのは、関数は static な定数として定義されるからである。

### 仕様の解釈

1. **通常の変数**: 親の関数スコープにはアクセス不可
2. **static 変数**: 親の関数スコープからもアクセス可能
3. **関数**: 暗黙的に static（Phase 3 で実装済み）
4. **グローバル変数**: 暗黙的に static（Phase 3 で実装済み）

### ユースケース

```nospace
func: outer() {
  static let: counter;    # 明示的 static #
  counter = 0;
  
  func: inner() {
    counter = counter + 1;  # static なのでアクセス可能 #
  }
  
  inner();
  inner();
  __assert(counter == 2);
}

func: main() {
  outer();
}
```

---

## 構文設計

### 選択肢

#### A: 修飾子を let の前に置く

```nospace
static let: x;
static let: y, z;   # y, z 両方が static #
```

**メリット**:
- 他言語（C, Rust など）に近い
- 修飾子が明確に宣言全体にかかる

**デメリット**:
- 新しいキーワードパターンが必要

#### B: 修飾子を let の後に置く

```nospace
let: static x;
let: static y, z;   # y のみ static? 両方? #
```

**メリット**:
- let: の後に続く形式で一貫性がある

**デメリット**:
- 複数変数宣言時の適用範囲が曖昧

#### C: 別のキーワードを使う

```nospace
staticlet: x;
slet: x;
```

**メリット**:
- 既存の構文に影響なし

**デメリット**:
- 新しいキーワードが増える
- 一般的な言語と異なる

### 決定: 選択肢 A を採用

`static let:` 構文を採用する。

```nospace
static let: x;
static let: y, z;   # y, z 両方が static #
```

---

## BNF 変更

```bnf
# 現行
let ::=
    | "let" ":" let_decl ("," let_decl)* ";"

# 変更後
let ::=
    | "static"? "let" ":" let_decl ("," let_decl)* ";"
```

---

## 現状分析

### Phase 3 で実装済みの基盤

1. **Variable.is_static フラグ**
   - 既に Variable 構造体に `is_static: bool` が存在
   - グローバル変数は暗黙的に `is_static = true`

2. **ScopeResolver の関数境界チェック**
   - 関数スコープ境界を越える場合、static 変数のみアクセス可能
   - この仕組みは Phase 3 で実装済み

3. **識別子解決ロジック**
   - `resolve_variable` で `is_static` をチェック
   - 非 static 変数は関数境界でスキップ

### 必要な変更

1. **token_parser**: `static` キーワードの追加
2. **tree_parser**: `static let:` 構文の解析
3. **Statement**: `VariableDeclaration` に static フラグを追加
4. **semantic_analyzer**: static フラグの伝播

---

## 詳細設計

### 1. token_parser の変更

#### 1.1 Keyword enum に Static を追加

```rust
// src/token_parser/mod.rs
pub enum Keyword {
    Let,
    Func,
    Return,
    If,
    Else,
    While,
    Break,
    Continue,
    Static,  // 追加
}
```

#### 1.2 キーワードマッチの追加

```rust
fn consume_keyword(iter: &mut impl Iterator<Item = char>) -> Option<Token> {
    // ...
    match id.as_str() {
        "let" => Some(Token::Keyword(Keyword::Let)),
        "func" => Some(Token::Keyword(Keyword::Func)),
        // ...
        "static" => Some(Token::Keyword(Keyword::Static)),  // 追加
        _ => Some(Token::Identifier(id)),
    }
}
```

### 2. tree_parser の変更

#### 2.1 Statement::VariableDeclaration に is_static を追加

```rust
// src/tree_parser/statement/mod.rs
pub enum Statement {
    VariableDeclaration(String, Box<Expression>, bool),  // (name, init, is_static)
    FunctionDeclaration(String, Vec<String>, Vec<LocatedStatement>),
    // ...
}
```

#### 2.2 parse_to_statements_let の変更

```rust
fn parse_to_statements_let(&mut self, start_pos: usize, is_static: bool) -> LocatedStatement {
    // ...
    LocatedStatement {
        statement: Statement::VariableDeclaration(id, init_expr, is_static),
        location: SourceLocation::new(start_pos, end_pos),
    }
}
```

#### 2.3 static let: の解析

```rust
fn parse_to_statement(&mut self) -> Option<LocatedStatement> {
    let token = self.iter.peek()?;
    let start_pos = token.token_info.code_pointer;
    
    match &token.token {
        Token::Keyword(Keyword::Static) => {
            self.iter.next(); // consume 'static'
            // 次が 'let' であることを確認
            if let Some(next) = self.iter.peek() {
                if matches!(next.token, Token::Keyword(Keyword::Let)) {
                    return Some(self.parse_to_statements_let(start_pos, true));
                }
            }
            // エラー: static の後に let がない
            // ...
        }
        Token::Keyword(Keyword::Let) => {
            Some(self.parse_to_statements_let(start_pos, false))
        }
        // ...
    }
}
```

### 3. semantic_analyzer の変更

#### 3.1 VariableDeclaration のパターンマッチ更新

```rust
Statement::VariableDeclaration(name, _, is_static) => {
    // Phase 3: グローバル変数は暗黙的に static
    // Phase 4: 明示的 static も考慮
    let final_is_static = *is_static || matches!(scope_type, ScopeType::Root);
    scope.add_variable(
        name,
        Variable {
            identifier: name.clone(),
            is_static: final_is_static,
        },
    )?;
}
```

---

## ネスト関数での static 変数

### テストケース

```nospace
func: counter_factory() {
  static let: count;
  count = 0;
  
  func: increment() {
    count = count + 1;  # static なのでアクセス可能 #
    return: count;
  }
  
  func: get() {
    return: count;  # static なのでアクセス可能 #
  }
  
  __assert(get() == 0);
  increment();
  __assert(get() == 1);
  increment();
  __assert(get() == 2);
}

func: main() {
  counter_factory();
}
```

### 注意点

1. **static 変数の寿命**
   - static 変数はスタックではなく、別の領域に保持する必要がある？
   - いいえ、nospace の現在の設計では、ネスト関数はその場で即時実行される
   - 関数オブジェクトとして外部に渡すことはできない（クロージャではない）
   - したがって、static 変数もスコープスタック内で管理可能

2. **スコープスタックの参照**
   - Phase 2 で実装した `scope_depth` による参照がそのまま使える
   - static 変数は関数境界を越えても参照可能

---

## 実行時の動作

### Phase 3 での実装（既存）

```rust
fn resolve_variable(&self, name: &str) -> Option<IdentifierRef> {
    let mut first_function_scope_depth: Option<usize> = None;
    
    for (depth, scope_info) in self.scope_stack.iter().rev().enumerate() {
        if scope_info.is_function_scope && first_function_scope_depth.is_none() {
            first_function_scope_depth = Some(depth);
        }
        
        if let Some(&local_index) = scope_info.var_indices.get(name) {
            let crossed_function_boundary = if let Some(first_func_depth) = first_function_scope_depth {
                depth > first_func_depth && scope_info.is_function_scope
            } else {
                false
            };
            
            // static 変数は関数境界を越えてもアクセス可能
            if crossed_function_boundary && !scope_info.variables[local_index].is_static {
                continue;
            }
            
            // ...
        }
    }
    None
}
```

この実装は Phase 3 で完成しており、Phase 4 では構文解析と `is_static` フラグの伝播のみが必要。

---

## 実装計画

### Step 1: token_parser の変更

1. Keyword::Static を追加
2. "static" をキーワードとして認識

### Step 2: tree_parser の変更

1. Statement::VariableDeclaration に is_static フラグを追加
2. `static let:` 構文の解析を実装
3. 既存のテストを更新

### Step 3: semantic_analyzer の変更

1. VariableDeclaration のパターンマッチを更新
2. is_static フラグを Variable に伝播

### Step 4: テストケースの追加

- `scope_static_001.ns`: 基本的な static 変数
- `scope_static_nested_001.ns`: ネスト関数からの static 変数アクセス
- `scope_static_error_001.ns`: 非 static 変数への関数境界越えアクセス（エラー）

### Step 5: ドキュメント更新

- BNF (docs/grammar.bnf) の更新
- spec.md の未実装フラグを削除

---

## 影響範囲

### 変更ファイル

1. **`src/token_parser/mod.rs`**
   - Keyword::Static 追加
   - キーワード認識

2. **`src/tree_parser/statement/mod.rs`**
   - Statement::VariableDeclaration の変更
   - static let: 構文解析

3. **`src/semantic_analyzer/mod.rs`**
   - VariableDeclaration のパターンマッチ更新

4. **`docs/grammar.bnf`**
   - let の BNF 更新

5. **`spec.md`**
   - 未実装フラグの削除

### 変更しないファイル

- `src/interpreter/mod.rs` - 変更不要（Phase 3 で基盤完成）

---

## テストケース

### 基本的な static 変数

```nospace
# scope_static_001.ns #
func: test() {
  static let: x;
  x = 42;
  
  func: inner() {
    __assert(x == 42);  # static なのでアクセス可能 #
  }
  
  inner();
}

func: main() {
  test();
}
```

### ネスト関数からの static 変数への書き込み

```nospace
# scope_static_nested_001.ns #
func: test() {
  static let: counter;
  counter = 0;
  
  func: inc() {
    counter = counter + 1;
  }
  
  inc();
  inc();
  __assert(counter == 2);
}

func: main() {
  test();
}
```

### static と非 static の混在

```nospace
# scope_static_mixed_001.ns #
func: test() {
  let: local;
  static let: shared;
  local = 10;
  shared = 20;
  
  func: inner() {
    # local にはアクセス不可（非 static） #
    __assert(shared == 20);  # static なのでアクセス可能 #
    shared = 30;
  }
  
  inner();
  __assert(shared == 30);
  __assert(local == 10);  # 変更されていない #
}

func: main() {
  test();
}
```

### エラーケース: 非 static 変数への関数境界越えアクセス

```nospace
# scope_static_error_001.ns (fails/) #
func: test() {
  let: local;  # 非 static #
  local = 10;
  
  func: inner() {
    local = 20;  # エラー: 非 static 変数への関数境界越えアクセス #
  }
}

func: main() {
  test();
}
```

---

## リスク

1. **構文解析の複雑化**
   - `static let:` という2トークンの組み合わせ
   - 解決: peek を使って先読み

2. **既存テストへの影響**
   - Statement::VariableDeclaration の変更による影響
   - 解決: 既存のテストを更新

3. **Phase 5 との関係**
   - ネスト関数の可視性ルールとの整合性
   - 解決: static 変数は関数より先に解決される

---

## 成功基準

- 全既存テストが通過
- `static let:` 構文が動作
- ネスト関数から static 変数にアクセス可能
- 非 static 変数への関数境界越えアクセスがエラー
- BNF と spec.md が更新済み

---

## Phase 3 との関係

Phase 3 で実装した以下の基盤をそのまま活用:
- `Variable.is_static` フラグ
- `ScopeResolver` の関数境界チェック
- `is_static` による条件分岐

Phase 4 の主な作業は構文解析層であり、実行時の動作は Phase 3 で完成している。

---

## 参考

### 関連ドキュメント

- [overview.md](overview.md) - スコープ実装の全体像
- [phase3-global-variables.md](phase3-global-variables.md) - Phase 3 の設計
