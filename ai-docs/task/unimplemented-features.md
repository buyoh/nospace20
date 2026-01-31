# 未実装の機能・構文

このドキュメントは nospace プログラミング言語における未実装の機能と構文をまとめたものです。

最終更新日: 2026-01-31

## 目次

1. [言語機能](#1-言語機能)
2. [構文要素](#2-構文要素)
3. [コンパイラ・実装](#3-コンパイラ実装)
4. [コード内の残件](#4-コード内の残件)
5. [テストケース](#5-テストケース)

---

## 1. 言語機能

### 1.1 型システム

**状態**: ❌ 未実装

**説明**: 型システムは完全に未実装。将来的に以下の型を導入予定。

- `int` : 整数
- `void` : 値なし  
- `function` : 関数
- `tuple` : タプル

**参照**:
- [spec.md](../../spec.md) セクション A

---

### 1.2 グローバル変数

**状態**: ❌ 未実装

**説明**: 現在は関数スコープ内でのみ変数定義が可能。グローバルスコープでの変数定義は未対応。

**構文例**:
```nospace
let:global_x;
global_x = 100;

func: main() {
  __clog(global_x);  # グローバル変数にアクセス #
}
```

**エラー**: `panic!("todo: global variable is not implemented")`

**参照**:
- [spec.md](../../spec.md) セクション 4, B
- [src/syntactic_analyzer/mod.rs](../../src/syntactic_analyzer/mod.rs#L203-L206)
- テスト: [disabled_var_global_001.ns](../../resources/tests/passes/variables/disabled_var_global_001.ns)

---

### 1.3 ブロックスコープ内での変数定義

**状態**: ❌ 未実装

**説明**: 現在、変数は関数スコープでのみ定義可能。if や while のブロック内で `let:` を使用することは未対応。

**構文例**:
```nospace
func: main() {
  let:x;
  x = 1;
  if: 1 {
    let:y;  # ブロック内で新しい変数を定義 #
    y = 2;
    x = x + y;
  };
  # y にはアクセスできない (スコープ外) #
}
```

**エラー**: `panic!("todo: block scoped variable is not implemented")`

**参照**:
- [spec.md](../../spec.md) セクション 4, 7, B
- [src/syntactic_analyzer/mod.rs](../../src/syntactic_analyzer/mod.rs#L199-L202)
- テスト: [disabled_scope_block_var_001.ns](../../resources/tests/passes/scope/disabled_scope_block_var_001.ns)

---

### 1.4 変数の初期値指定

**状態**: ❌ 未実装

**説明**: 変数定義時に初期値を指定する機能は未実装。現在は全ての変数が 0 で初期化される。

**構文例**:
```nospace
func: main() {
  let:x = 10;      # 初期値 10 #
  let:y = x * 2;   # 初期値は式でも可 #
}
```

**課題**: 
- グローバルスコープに記述された初期値はいつ実行されるか？ (TODO)

**参照**:
- [spec.md](../../spec.md) セクション 4
- テスト: [disabled_var_init_001.ns](../../resources/tests/passes/variables/disabled_var_init_001.ns)

---

### 1.5 final / const 変数

**状態**: ❌ 未実装

**説明**: 再代入不可の変数を定義する機能は未実装。

**構文例**:
```nospace
func: main() {
  final:x;   # 再代入不可 #
  x = 10;
  # x = 20;  # エラー: 再代入不可 #
  
  const:PI = 3;  # リテラルのみ代入可かつ再代入不可 #
}
```

**参照**:
- [spec.md](../../spec.md) セクション 4
- テスト: [disabled_var_final_001.ns](../../resources/tests/passes/variables/disabled_var_final_001.ns)

---

### 1.6 static 変数

**状態**: ❌ 未実装

**説明**: 親の関数スコープにアクセス可能な static 変数は未実装。

**参照**:
- [spec.md](../../spec.md) セクション 7

---

### 1.7 16進数リテラル

**状態**: ❌ 未実装

**説明**: 16進数リテラル (`0x...`) は未対応。現在は10進整数のみサポート。

**構文例**:
```nospace
let:x = 0xFF;   # 未実装 #
let:y = 0x10;   # 未実装 #
```

**参照**:
- [spec.md](../../spec.md) セクション B

---

### 1.8 if/while 式の戻り値

**状態**: ⚠️ 制限あり

**説明**: if と while は式として使用可能だが、常に 0 を返す。将来的には評価した値を返すように改善予定。

**現状**:
```nospace
x = if: cond { 5 } else: { 10 };  # x は常に 0 #
```

**TODO**: 評価した値を返す

**参照**:
- [spec.md](../../spec.md) セクション 6.1, 6.2
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

---

### 1.9 return なし関数の戻り値

**状態**: ⚠️ 仕様検討中

**説明**: `return:` がない場合、関数は値を返さない (`None`)。この挙動は要検討。

**TODO**: 仕様を確定させる

**参照**:
- [spec.md](../../spec.md) セクション 5

---

## 2. 構文要素

### 2.1 else if

**状態**: ✅ 実装済み (制限付き)

**説明**: `else if` は `else: if: 条件式 { ... }` の形式で記述可能。専用の構文は存在しない。

**構文例**:
```nospace
if: x == 1 {
  # ... #
} else: if: x == 2 {
  # ... #
} else: {
  # ... #
};
```

**参照**:
- [spec.md](../../spec.md) セクション 6.2

---

## 3. コンパイラ・実装

### 3.1 コンパイラ (compiler)

**状態**: ❌ 未実装

**説明**: コンパイラモジュールは完全に未実装。現在はインタプリタのみ動作。

**ファイル**: [src/compiler/mod.rs](../../src/compiler/mod.rs)

**内容**: `// todo!` のみ

**参照**:
- [ai-docs/architecture/overview.md](../architecture/overview.md#5-compiler-コンパイラ---未実装)
- [ai-docs/architecture/modules.md](../architecture/modules.md#compiler)
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

---

### 3.2 grayspace ターゲット

**状態**: ❌ 未実装

**説明**: grayspace ターゲットへのコンパイルは未実装。

**ディレクトリ**: `src/compiler/grayspace/` (存在するが未実装)

---

## 4. コード内の残件

### 4.1 Expression::Invalid の処理

**状態**: ❌ 未実装

**場所**: [src/syntactic_analyzer/mod.rs](../../src/syntactic_analyzer/mod.rs#L65)

**コード**:
```rust
Expression::Invalid(_) => todo!(),
```

**説明**: パース時にエラーとなった Invalid な式の処理が未実装。

**参照**:
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md#syntactic_analyzer)

---

### 4.2 Clone derive の削除 (最適化)

**状態**: ⚠️ TODO

**場所**: [src/syntactic_analyzer/mod.rs](../../src/syntactic_analyzer/mod.rs)

**コード**:
```rust
// #[derive(Clone)] // TODO: REMOVE
pub enum ExecExpression { ... }

// #[derive(Clone)] // TODO: REMOVE
pub enum ExecStatement { ... }
```

**説明**: パフォーマンス最適化のため、不要な `Clone` derive を削除予定。

---

### 4.3 変数/関数の識別子管理の改善

**状態**: ⚠️ TODO

**場所**: [src/syntactic_analyzer/mod.rs](../../src/syntactic_analyzer/mod.rs)

**コード**:
```rust
pub struct Variable {
    pub identifier: String, // TODO: use IdentifierInfo
}

pub struct Function {
    pub args: Vec<String>, // TODO: change string to identifier_ptr
    ...
}

struct IdentifierInfo {
    idx: usize, // TODO: more safety
}
```

**説明**: 
- 文字列ベースの識別子管理を `IdentifierInfo` ベースに変更予定
- より安全な型に変更予定

---

### 4.4 エラーメッセージ型の改善

**状態**: ⚠️ 検討中

**説明**: `CodeParseError.message` を `Cow<'static, str>` に変更することを検討中。

---

## 5. テストケース

### 5.1 無効化されたテスト (disabled_*)

以下のテストは未実装機能のため無効化されています:

| テストファイル | 機能 | 状態 |
|---------------|------|------|
| [disabled_var_global_001.ns](../../resources/tests/passes/variables/disabled_var_global_001.ns) | グローバル変数 | ❌ 未実装 |
| [disabled_var_init_001.ns](../../resources/tests/passes/variables/disabled_var_init_001.ns) | 変数初期値指定 | ❌ 未実装 |
| [disabled_var_final_001.ns](../../resources/tests/passes/variables/disabled_var_final_001.ns) | final 変数 | ❌ 未実装 |
| [disabled_scope_block_var_001.ns](../../resources/tests/passes/scope/disabled_scope_block_var_001.ns) | ブロックスコープ変数 | ❌ 未実装 |

**参照**:
- [ai-docs/task/test-categorization.md](./test-categorization.md)

---

### 5.2 失敗しているテスト

一部のテストは未実装機能に依存しているため失敗しています:

- `c004.ns` - ブロックスコープ内変数定義が必要
- `scope_block_001.ns` - ブロックスコープ内変数定義が必要
- `integration_integ_001.ns` - 複数の未実装機能を含む

**参照**:
- [ai-docs/task/test-categorization.md](./test-categorization.md#現状の問題点)

---

## 6. 優先度

### 高優先度

1. **ブロックスコープ内での変数定義** - 多くのテストがこれに依存
2. **Expression::Invalid の処理** - エラーハンドリングの完全性のため
3. **変数初期値指定** - 基本的な利便性向上

### 中優先度

4. **final/const 変数** - 安全性向上
5. **グローバル変数** - より柔軟なプログラム構造のため
6. **if/while 式の戻り値改善** - より表現力の高い言語のため

### 低優先度

7. **16進数リテラル** - 利便性向上
8. **static 変数** - 高度な機能
9. **型システム** - 大規模な設計・実装が必要
10. **コンパイラ** - 大規模な実装が必要

---

## 7. 関連ドキュメント

- [spec.md](../../spec.md) - 言語仕様
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md) - 実装状況の詳細
- [ai-docs/task/test-categorization.md](./test-categorization.md) - テスト分類
- [ai-docs/task/test-error-handling.md](./test-error-handling.md) - エラーハンドリングテスト
- [ai-docs/architecture/overview.md](../architecture/overview.md) - アーキテクチャ概要

---

## 更新履歴

- 2026-01-31: 初版作成
