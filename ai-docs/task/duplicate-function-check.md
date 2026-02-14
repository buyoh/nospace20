# 同一スコープ内の関数重複定義の検出

## 概要

同一スコープ内で同じ名前の関数を複数定義した場合に、コンパイルエラーとして検出する機能を実装する。

## 現状の問題

### 1. 関数重複定義が検出されない

現在の実装では、関数宣言時に重複チェックが行われていない。

[src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs#L272-L301) のパス1aで、関数を `scope.identifier_map.insert()` を使って直接登録している：

```rust
// identifier_map にはグローバルインデックスを登録
scope.identifier_map.insert(
    name.clone(),
    Identifier::Function(FunctionIndex(global_idx)),
);
```

`BTreeMap::insert()` は既存のキーがあっても上書きするため、重複が検出されない。

### 2. 変数との名前衝突も検出されない可能性

関数は変数より先に登録される（パス1a→パス1b）ため、変数の重複チェックで関数との衝突が検出される。

しかし逆のパターン（変数が先、関数が後）は現在の実装では発生しないため問題ないが、将来の変更で問題になる可能性がある。

## 設計

### 対応方針

変数の重複チェックと同様に、関数登録時にも `add_identifier()` を使用する方式に統一する。

### 実装箇所

#### 1. semantic_analyzer/mod.rs の修正

[src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs#L272-L301) のパス1aを修正：

**変更前:**
```rust
// identifier_map にはグローバルインデックスを登録
scope.identifier_map.insert(
    name.clone(),
    Identifier::Function(FunctionIndex(global_idx)),
);
```

**変更後:**
```rust
// identifier_map にはグローバルインデックスを登録
scope.add_identifier(
    name,
    Identifier::Function(FunctionIndex(global_idx)),
)?;
```

#### 2. エラー処理の統一

`scope.add_identifier()` は既に重複チェックを実装しており、以下のエラーメッセージを返す：

```
semantic error: the name '{}' is already used
```

このエラーメッセージは変数と関数の両方で統一される。

### エラーケース

以下のケースがエラーとして検出されるようになる：

#### ケース1: 同一スコープ内の関数重複定義

```nospace
func: foo() { 
  __puti(1); 
}

func: foo() {  # エラー: foo は既に定義されている #
  __puti(2); 
}

func: main() {}
```

#### ケース2: グローバルスコープでの関数重複

```nospace
let: x;

func: foo() { 
  __puti(1); 
}

func: foo() {  # エラー: foo は既に定義されている #
  __puti(2); 
}

func: main() {}
```

#### ケース3: ネストされた関数スコープでの重複

```nospace
func: outer() {
  func: inner() {  # OK: outer スコープで定義 #
    __puti(1);
  }
  
  func: inner() {  # エラー: inner は既に outer スコープで定義されている #
    __puti(2);
  }
}

func: main() {}
```

#### ケース4: 関数と変数の名前衝突（既に検出される）

```nospace
func: foo() {  # OK #
  __puti(1);
}

let: foo;  # エラー: foo は既に使用されている #

func: main() {}
```

このケースは現在の実装でも検出される（変数定義時に `add_variable` → `add_identifier` で重複チェック）。

## テストケース

### 追加するテストファイル

#### 1. グローバルスコープでの関数重複

- **ファイル**: `resources/tests/fails/compile/func_duplicate_global_001.ns`
- **内容**: グローバルスコープで同じ名前の関数を2回定義

```nospace
# エラーケース: グローバルスコープでの関数の重複定義 #

func: foo() {
  __puti(1);
}

func: foo() {
  __puti(2);
}

func: main() {
  foo();
}
```

- **期待するエラー**: `semantic error: the name 'foo' is already used`

#### 2. ネストされたスコープでの関数重複

- **ファイル**: `resources/tests/fails/compile/func_duplicate_nested_001.ns`
- **内容**: ネストされた関数スコープ内で同じ名前の関数を2回定義

```nospace
# エラーケース: ネストスコープでの関数の重複定義 #

func: outer() {
  func: inner() {
    __puti(1);
  }
  
  func: inner() {
    __puti(2);
  }
  
  inner();
}

func: main() {
  outer();
}
```

- **期待するエラー**: `semantic error: the name 'inner' is already used`

#### 3. 関数と変数の名前衝突（逆順パターン）

既存の変数重複テストで十分カバーされているが、明示的に追加してもよい：

- **ファイル**: `resources/tests/fails/compile/func_var_conflict_001.ns`
- **内容**: 関数と同じ名前の変数を定義

```nospace
# エラーケース: 関数と変数の名前衝突 #

func: foo() {
  __puti(1);
}

let: foo;

func: main() {
  foo();
}
```

- **期待するエラー**: `semantic error: the name 'foo' is already used`

#### 4. main 関数の重複

- **ファイル**: `resources/tests/fails/compile/func_duplicate_main_001.ns`
- **内容**: main 関数を2回定義

```nospace
# エラーケース: main 関数の重複定義 #

func: main() {
  __puti(1);
}

func: main() {
  __puti(2);
}
```

- **期待するエラー**: `semantic error: the name 'main' is already used`

### test-manifest.yaml への追加

`resources/tests/test-manifest.yaml` に以下のエントリを追加：

```yaml
- name: test_compile_error_func_duplicate_global_001
  type: compile_error
  path: func_duplicate_global_001
  comment: "Error: duplicate function definition in global scope"

- name: test_compile_error_func_duplicate_nested_001
  type: compile_error
  path: func_duplicate_nested_001
  comment: "Error: duplicate function definition in nested scope"

- name: test_compile_error_func_var_conflict_001
  type: compile_error
  path: func_var_conflict_001
  comment: "Error: function-variable name conflict"

- name: test_compile_error_func_duplicate_main_001
  type: compile_error
  path: func_duplicate_main_001
  comment: "Error: duplicate main function definition"
```

## 実装手順

1. **ソースコード修正**
   - [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs#L297-L301) で `insert` を `add_identifier` に変更

2. **テストファイル追加**
   - 上記4つのテストケースを `resources/tests/fails/compile/` に追加
   - 各テストに対応する `.check.json` ファイルを作成

3. **test-manifest.yaml 更新**
   - 4つのテストエントリを追加

4. **テスト実行**
   - `cargo test` で全テストが通ることを確認
   - 新規追加したテストが期待通りエラーを検出することを確認

## 影響範囲

### 変更されるファイル

- `src/semantic_analyzer/mod.rs` - 関数登録のロジックを1行変更

### 影響を受ける機能

- 関数の重複定義が行われている既存コードはエラーになる
  - 現在のテストケースには関数重複定義は含まれていないため、既存テストへの影響はないと予想

### 互換性

- **後方互換性**: なし（重複定義していたコードはエラーになる）
- **仕様との整合性**: 改善（他のプログラミング言語と同様、重複定義を禁止することで一貫性が向上）

## 備考

### 関連する既存の実装

- [src/semantic_analyzer/scope.rs](../../src/semantic_analyzer/scope.rs#L325-L337) の `add_identifier()` メソッド
  - 既に重複チェックのロジックを実装している
  - 変数の重複チェックでも使用されている

- [src/semantic_analyzer/scope.rs](../../src/semantic_analyzer/scope.rs#L338-L344) の `add_variable()` メソッド
  - 内部で `add_identifier()` を呼び出している
  - 同様のパターンを関数登録でも適用する

### 将来の拡張

- 関数のオーバーロードをサポートする場合、この実装を拡張する必要がある
  - ただし、現在の仕様にはオーバーロードの記載がないため、当面は重複禁止で問題ない

### エラーメッセージの改善（オプション）

現在のエラーメッセージは汎用的だが、より詳細にすることも可能：

- 変数と関数を区別: `"semantic error: the variable '{}' is already defined"`
- 再定義の場所を表示: `"semantic error: the name '{}' is already used at line X"`

これらは将来的な改善として検討可能。

## 完了条件

- [ ] `src/semantic_analyzer/mod.rs` の修正
- [ ] 4つのテストファイルの追加
- [ ] `test-manifest.yaml` の更新
- [ ] `cargo test` で全テストが成功
- [ ] `cargo test --test code_test` で新規テストがエラーを検出することを確認
