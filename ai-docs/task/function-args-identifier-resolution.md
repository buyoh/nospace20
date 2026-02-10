# Function の args フィールドにおける識別子解決の考察

分離元: [technical-debt.md](./technical-debt.md) セクション 3.2

作成日: 2026-02-10

## 背景

`scope::Function` 構造体は semantic_analyzer の出力であり、関数のメタ情報を保持する。
現在 `args` フィールドは `Vec<String>` であり、引数名を文字列で保持している。

```rust
// src/semantic_analyzer/scope.rs
pub struct Function {
    pub args: Vec<String>,        // ← 文字列で保持
    pub arg_indices: Vec<usize>,  // ← 事前計算済みインデックス
    pub block: Block,
}
```

semantic analyzer の出力では識別子は最終的に全て解決されているべきであり、`args` が文字列のまま残っているのは設計上の不整合である。

---

## 現状の使用箇所の分析

### 1. 構築箇所: semantic_analyzer/mod.rs (L338〜L366)

```rust
Statement::FunctionDeclaration(name, args, block) => {
    // analyze_internal_with_parent に args.clone() を渡す
    // → 引数は initial_vars として ScopeBuilder に登録される
    let (s, es) = analyze_internal_with_parent(
        block, ScopeType::Function, args.clone(), Some(&resolver)
    )?;
    let built_scope = s.build(true, Vec::new());

    // arg_indices を事前計算
    let arg_indices: Vec<usize> = args.iter().map(|arg_name| {
        *built_scope.variable_indices.get(arg_name).expect(...)
    }).collect();

    let func = Function {
        args: args.clone(),    // TreeParser の文字列をそのまま保持
        arg_indices,
        block: Block { scope: built_scope, statements: es },
    };
    scope.add_function(name, func)?;
}
```

**ポイント**: `args` は `arg_indices` の計算に使われた後、そのまま `Function` に格納される。`arg_indices` が計算された後は、構築側では `args` は不要。

### 2. 使用箇所: interpreter/exec.rs (L26〜L38, L230〜L234)

```rust
// new_func (L26)
let mut variables = vec![0; func.block.scope.variable_count];
for (i, arg_val) in args.iter().enumerate() {
    if i < func.arg_indices.len() {
        variables[func.arg_indices[i]] = *arg_val;
    }
}
```

```rust
// interpret_call_function (L230)
for (i, arg_val) in arg_values.iter().enumerate() {
    if i < func.arg_indices.len() {
        variables[func.arg_indices[i]] = *arg_val;
    }
}
```

**ポイント**: インタプリタは `func.args` を**一切参照していない**。`func.arg_indices` のみ使用。

### 3. 使用箇所: compiler_ws/statement.rs (L116〜L128)

```rust
for i in (0..func.args.len()).rev() {
    let offset = func.arg_indices.get(i).copied().unwrap_or(i) as i64;
    // ...
}
```

**ポイント**: `func.args.len()` のみ参照している。引数の「数」が必要なだけで、引数の「名前」は不要。

---

## 問題の整理

| 消費者 | args（文字列） | arg_indices（インデックス） | 備考 |
|--------|:-:|:-:|------|
| semantic_analyzer (構築時) | ○ | ○ | arg_indices 計算に名前を使用 |
| interpreter | × | ○ | 名前不要 |
| compiler_ws | △ (len のみ) | ○ | `.len()` のみ。名前不要 |

### 核心的な問い

1. **`args` を完全に削除できるか？**
   - インタプリタ: `arg_indices` だけで動作する → 削除可能
   - compiler_ws: `args.len()` は `arg_indices.len()` で代替可能 → 削除可能
   - → **`args` フィールドは削除可能**

2. **`args` を削除すべきか、型を変更すべきか？**
   - 現在 `arg_indices` は `Vec<usize>` であり、引数の数 = `arg_indices.len()`
   - `args` の持つ情報で `arg_indices` に含まれていないものは「引数名」のみ
   - 引数名はデバッグやエラーメッセージで将来的に必要になるかもしれないが、現状は不要

3. **`arg_indices` の妥当性**
   - `arg_indices` は既に最適化のために存在しており、実質的に識別子解決済みの情報
   - `arg_indices[i]` = i番目の引数の、関数スコープ内でのスロットインデックス
   - これは「引数の識別子が解決された結果」そのものである

---

## 設計案

### 案A: args を削除し、arg_indices のみ残す（推奨）

```rust
pub struct Function {
    pub arg_indices: Vec<usize>,
    pub block: Block,
}
```

- **メリット**: 最もシンプル。不要な文字列保持を排除
- **影響範囲**:
  - `compiler_ws/statement.rs`: `func.args.len()` → `func.arg_indices.len()` に変更（1箇所）
  - 構築時: `args.clone()` の格納を削除
- **リスク**: 将来引数名が必要になったら `block.scope` から取得可能（`block.scope.variables` に引数が含まれている）

### 案B: args を arg_count: usize に変更

```rust
pub struct Function {
    pub arg_count: usize,
    pub arg_indices: Vec<usize>,
    pub block: Block,
}
```

- **メリット**: 引数の個数という概念を明示的に保持
- **デメリット**: `arg_count == arg_indices.len()` は常に成立するため冗長
- **不採用理由**: 冗長な情報

### 案C: 現状維持（非推奨）

- **デメリット**: semantic analyzer の出力に未解決の文字列識別子が残る

---

## 推奨方針

**案A（args フィールドの削除）を推奨する。**

### 根拠

1. `args` は semantic analyzer の出力としては未解決の識別子であり、設計上不整合
2. `arg_indices` が既に完全な識別子解決結果を保持している
3. 全ての消費者（interpreter, compiler_ws）が `arg_indices` で動作可能
4. compiler_ws での `func.args.len()` は `func.arg_indices.len()` で代替可能
5. 引数名が将来必要になった場合は `func.block.scope.variables` から取得可能
   （引数は `scope.variables` の先頭に順に登録されている）

### 変更量の見積もり

- 変更ファイル: 2ファイル
  - `src/semantic_analyzer/scope.rs`: `Function` から `args` フィールド削除
  - `src/semantic_analyzer/mod.rs`: `Function` 構築時に `args: args.clone()` 削除
  - `src/compiler_ws/statement.rs`: `func.args.len()` → `func.arg_indices.len()`
- 影響: 小（数行の変更）
- テスト: 既存テスト全パスで確認可能

---

## 関連する技術的負債

- **Variable.identifier (3.1)**: 同様に文字列を保持しているが、`Variable` はスコープのメタ情報として使用されるため、用途が異なる。`Function.args` とは独立して検討すべき。
- **ExecExpression::Function の String (types.rs)**: 関数呼び出し式が関数名を文字列で保持している点も未解決。ただしこれはユーザー定義関数と組み込み関数の解決に関わる別の課題。

---

## デバッグ用シンボルテーブルによる識別子名管理

semantic analyzer の出力全体から文字列識別子を分離し、インデックスから識別子名を逆引きできる
SymbolTable を別構造体で持つ設計についての詳細な考察は、別ドキュメントに分離した。

→ [symbol-table-design.md](./symbol-table-design.md)

---

## 更新履歴

- 2026-02-10: シンボルテーブル考察を symbol-table-design.md に分離
- 2026-02-10: デバッグ用シンボルテーブルによる識別子名管理の検討を追加
- 2026-02-10: 初版作成
