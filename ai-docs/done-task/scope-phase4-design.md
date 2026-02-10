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

### 構文

`static:` は `let:` や `func:` と同様のキーワード構文を使用する。

```nospace
static: a;
static: x, y;   # x, y 両方が static #
```

**注意**: `static let:` ではない。`static:` 単体がキーワードである。

### 初期化タイミング

- static 変数は **main が呼び出される前** に初期化される（グローバル変数と同じタイミング）
- static 変数が定義された関数が呼び出されても **再初期化されない**
- 初期化には **変数と定数以外は使用できない**（リテラルや定数のみ）
- **static 変数は、static でないグローバル変数より先に初期化される**

### ユースケース

```nospace
let: global_var;  # グローバル変数 #

func: setter() {
  static: static_var;  # static変数 #
  static_var += 1;
  __clog(static_var);  # 呼び出される度に 1, 2, 3, ... と増加 #
  global_var = static_var;  # グローバル変数に代入 #
}

func: main() {
  setter();
  __assert(global_var == 1);
  setter();
  __assert(global_var == 2);
}
```

---

## BNF 変更

```bnf
# 現行
let ::=
    | "let" ":" let_decl ("," let_decl)* ";"

# 追加
static ::=
    | "static" ":" let_decl ("," let_decl)* ";"
```

`static:` は `let:` と同じ形式の宣言構文であり、`let` の修飾子ではなく独立したキーワードである。

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

1. **token_parser**: `static` キーワードの追加（`let` と同様の位置づけ）
2. **tree_parser**: `static:` 構文の解析（`let:` と同様の形式）
3. **Statement**: `VariableDeclaration` に static フラグを追加
4. **semantic_analyzer**: static フラグの伝播
5. **interpreter / compiler**: static 変数の初期化順序の実装

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

#### 2.2 `static:` の解析

`static:` は `let:` と同じ形式で解析する。独立したキーワードとして扱い、コロンの後に変数名リストが続く。

```rust
fn parse_to_statement(&mut self) -> Option<LocatedStatement> {
    let token = self.iter.peek()?;
    let start_pos = token.token_info.code_pointer;
    
    match &token.token {
        Token::Keyword(Keyword::Static) => {
            self.iter.next(); // consume 'static'
            // ':' を消費（let: と同様）
            self.expect_colon();
            return Some(self.parse_to_statements_let_like(start_pos, /* is_static = */ true));
        }
        Token::Keyword(Keyword::Let) => {
            self.iter.next();
            self.expect_colon();
            Some(self.parse_to_statements_let_like(start_pos, /* is_static = */ false))
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

### 4. 初期化順序の実装

static 変数は main 呼び出し前に初期化される必要がある。

#### 初期化の順序

1. **static 変数** の初期化（変数と定数のみで初期化可能）
2. **グローバル変数** の初期化
3. **main()** の呼び出し

#### interpreter での実装

- プログラム開始時に AST を走査し、全関数内の `static:` 宣言を収集
- static 変数を先に初期化
- その後、通常のグローバル変数を初期化
- main() を呼び出す

#### compiler での実装

- コンパイル時に static 変数の初期化コードをグローバル変数より前に配置

---

## ネスト関数での static 変数

### テストケース

```nospace
func: counter_factory() {
  static: count;
  # count は main 前に 0 で初期化済み。
  # 関数呼び出し時に再初期化されない。

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

1. **static 変数の寿命と初期化**
   - static 変数は main 前に初期化されるため、関数呼び出し時には再初期化されない
   - 関数が複数回呼ばれても値は保持される
   - 初期化には定数・リテラルのみ使用可能

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

この実装は Phase 3 で完成しており、Phase 4 では構文解析と `is_static` フラグの伝播、および初期化順序の実装が必要。

---

## 実装計画

### Step 1: token_parser の変更

1. Keyword::Static を追加
2. "static" をキーワードとして認識

### Step 2: tree_parser の変更

1. `static:` 構文の解析を実装（`let:` と同様の独立キーワード形式）
2. Statement に is_static フラグを追加
3. 既存のテストを更新

### Step 3: semantic_analyzer の変更

1. VariableDeclaration のパターンマッチを更新
2. is_static フラグを Variable に伝播

### Step 4: 初期化順序の実装

1. static 変数を main 前に初期化するロジック
2. static 変数 → グローバル変数 の順序を実装
3. 初期化に定数・リテラル以外を使用した場合のエラー処理

### Step 5: テストケースの追加

- `scope_static_001.ns`: 基本的な static 変数
- `scope_static_persist_001.ns`: 関数呼び出し間での値保持
- `scope_static_nested_001.ns`: ネスト関数からの static 変数アクセス
- `scope_static_init_order_001.ns`: 初期化順序の確認
- `scope_static_error_001.ns`: 非 static 変数への関数境界越えアクセス（エラー）

### Step 6: ドキュメント更新

- BNF (docs/grammar.bnf) の更新
- spec.md の未実装フラグを削除

---

## 影響範囲

### 変更ファイル

1. **`src/token_parser/mod.rs`**
   - Keyword::Static 追加
   - キーワード認識

2. **`src/tree_parser/statement/mod.rs`**
   - `static:` 構文解析（`let:` と同様の独立キーワード）

3. **`src/semantic_analyzer/mod.rs`**
   - VariableDeclaration のパターンマッチ更新

4. **`src/interpreter/mod.rs`**
   - static 変数の main 前初期化ロジック
   - 初期化順序（static → global）

5. **`src/compiler_ws/`** (該当する場合)
   - static 変数の初期化コード生成

6. **`docs/grammar.bnf`**
   - `static:` の BNF 追加

7. **`spec.md`**
   - 未実装フラグの削除

### 変更しないファイル

- スコープ解決のランタイムロジック（Phase 3 で完成済み）

---

## テストケース

### 基本的な static 変数

```nospace
# scope_static_001.ns #
func: test() {
  static: x;
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

### static 変数の値保持（関数呼び出し間）

```nospace
# scope_static_persist_001.ns #
func: counter() {
  static: count;
  count = count + 1;
  return: count;
}

func: main() {
  __assert(counter() == 1);
  __assert(counter() == 2);
  __assert(counter() == 3);
}
```

### ネスト関数からの static 変数への書き込み

```nospace
# scope_static_nested_001.ns #
func: test() {
  static: counter;

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
  static: shared;
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

### 初期化順序

```nospace
# scope_static_init_order_001.ns #
let: g;

func: f() {
  static: s;
  # s は g より先に 0 で初期化されている #
}

func: main() {
  f();
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

1. **初期化順序の実装**
   - static 変数を main 前に初期化する仕組みが必要
   - AST 走査で全関数内の static 宣言を収集する必要がある
   - 解決: プログラム開始時のパスで static 宣言を収集・初期化

2. **初期化制約**
   - static 変数は変数と定数以外で初期化できない
   - 解決: semantic_analyzer で初期化式のバリデーション

3. **既存テストへの影響**
   - Statement の変更による影響
   - 解決: 既存のテストを更新

4. **Phase 5 との関係**
   - ネスト関数の可視性ルールとの整合性
   - 解決: static 変数は関数より先に解決される

---

## 成功基準

- 全既存テストが通過
- `static:` 構文が動作
- static 変数が main 前に初期化される
- static 変数が非 static グローバル変数より先に初期化される
- 関数呼び出し間で static 変数の値が保持される
- ネスト関数から static 変数にアクセス可能
- 非 static 変数への関数境界越えアクセスがエラー
- BNF と spec.md が更新済み

---

## Phase 3 との関係

Phase 3 で実装した以下の基盤をそのまま活用:
- `Variable.is_static` フラグ
- `ScopeResolver` の関数境界チェック
- `is_static` による条件分岐

Phase 4 の主な作業は:
1. `static:` 構文の解析（独立キーワード、`let:` と同様の形式）
2. is_static フラグの伝播
3. 初期化順序の実装（static → global → main）
4. 初期化式の制約チェック

---

## 参考

### 関連ドキュメント

- [overview.md](overview.md) - スコープ実装の全体像
- [phase3-global-variables.md](phase3-global-variables.md) - Phase 3 の設計
