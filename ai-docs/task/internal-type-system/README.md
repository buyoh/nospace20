# 内部型システム（int / void）の導入

## 概要

明示的な型定義構文は導入しないが、コンパイラ内部で `int` と `void` の2つの型を管理し、不正な型の使用をコンパイルエラーとして検出する。

## 目的

- `while` や `if`（else なし）の戻り値を式として使用するケースを静的に禁止
- ユーザー定義関数の戻り値の型を自動推論し、return のない関数を void として扱う
- void 式を値が必要な文脈（代入、演算、引数等）で使用した場合にエラーを報告

## 型ルール

| 式 | 型 |
|----|-----|
| 整数リテラル | int |
| 変数 | int |
| 二項演算（`+`, `-`, `*`, `/`, `%`, `==`, etc.） | int（両辺が int であること） |
| 単項演算（`-`, `!`, `*`） | int |
| 参照（`&`） | int |
| 配列アクセス（`arr[i]`） | int |
| `while: ... { ... }` | **void** |
| `if: ... { ... }` (else なし) | **void** |
| `if: ... { A } else: { B }` | A, B が共に int なら int。いずれかが void なら **void** |
| `{ ... }` (ブロック式) | 最後の式文の型。空ブロック → **void** |
| ユーザー関数呼び出し | 関数の戻り値型（int または void） |
| 組み込み関数呼び出し | 関数ごとに固定（後述） |

### 組み込み関数の戻り値型

| 関数 | 戻り値型 |
|------|----------|
| `__puti(x)` | int（引数値を返す） |
| `__putc(x)` | int（引数値を返す） |
| `__geti()` | int |
| `__getc()` | int |
| `__clog(x)` | int（引数値を返す） |
| `__assert(x)` | int（引数値を返す） |
| `__assert_not(x)` | int（引数値を返す） |
| `__trace(x)` | void |

注: `__trace` は void にする。テスト用トレース関数であり戻り値を使用する意味がないため。

### ユーザー定義関数の戻り値型推論

- 関数本体に `return: expr;` が1つ以上ある → **int**
- 関数本体に `return:` がない（暗黙の return）→ **void**
- `return: expr;` と暗黙 return が混在 → **エラー**（具体的には、ある制御パスで return あり、別のパスで return なしの場合）

注: 簡易実装として、関数本体のステートメントを走査して `return:` 文が存在するかだけを確認する。制御フロー解析（到達可能性分析）は行わない。

### void 式が使用できない文脈

以下の文脈で void 型の式を使用するとコンパイルエラー:

1. 代入の右辺: `x = <void式>;`
2. 二項演算のオペランド: `<void式> + 1`
3. 単項演算のオペランド: `-<void式>`
4. 関数の引数: `foo(<void式>)`
5. 組み込み関数の引数: `__assert(<void式>)`
6. `return:` の式: `return: <void式>;`（int 関数内）
7. 条件式: `if: <void式> { ... }`, `while: <void式> { ... }`
8. 配列インデックス: `arr[<void式>]`

### void 式が使用できる文脈

1. 式文（文としての使用）: `<void式>;` — OK（値は破棄される）
2. ブロックの最後の式: `{ ...; <void式>; }` — ブロック全体が void になる
3. if/else のブロック: `if: cond { <void式>; } else: { <void式>; };` — if 全体が void になる

## ドキュメント構成

- [overview.md](overview.md) - 設計概要・フェーズ分割
- [semantic-analyzer-changes.md](semantic-analyzer-changes.md) - semantic_analyzer の変更設計
- [interpreter-changes.md](interpreter-changes.md) - interpreter の変更設計
- [compiler-ws-changes.md](compiler-ws-changes.md) - compiler_ws の変更設計
- [test-changes.md](test-changes.md) - テストの修正・追加計画

## 実装状況

### Phase 1: 型定義と型推論の基盤 ✅

- `src/semantic_analyzer/types.rs`: `ValueType` enum, `ExecExpression::infer_type()`, `infer_block_type()`, `ValueType::merge()` を追加
- `src/semantic_analyzer/scope.rs`: `FunctionIndex` に `ValueType` 追加、`Function` に `return_type` 追加、`ScopeResolver::get_function_return_type()` 追加

### Phase 2: 型チェック ✅

- `src/semantic_analyzer/mod.rs`:
  - `has_return_statement()`, `expr_contains_return()` で return 文の存在確認
  - `guarantees_return()`, `expr_guarantees_return()` で全パスの return 保証チェック（軽量制御フロー解析）
  - `require_int_type()` で void 式の値使用を検出
  - `convert_to_exec_expression_with_resolver()` で全 void-unsafe 文脈をチェック
  - `analyze_internal_with_parent()` に `inherited_func_return_types` パラメータ追加
  - パス1a で関数戻り値型推論（has_return_statement + guarantees_return）
  - mixed return（return ありだが全パス保証なし）をエラーとして検出

### Phase 3: interpreter 対応 ✅

- 変更不要。semantic_analyzer が型安全性を保証するため、interpreter は従来どおり `i64` で動作

### Phase 4: compiler_ws 対応（最小）

- `src/compiler_ws/context.rs`: `collect_func_return_types()` メソッド追加
- void 式の最適化（スタック操作省略）は未実装。semantic 正確性は Phase 2 で保証済みのため、void 式は内部的に 0 をプッシュし Discard される従来の動作を維持

### Phase 5: テスト修正・追加 ✅

#### 新規テスト（compile_error）
- `void_while_assign_001`, `void_if_no_else_assign_001`, `void_func_assign_001`
- `void_in_operation_001`, `void_in_condition_001`, `void_func_mixed_return_001`

#### 新規テスト（success）
- `void_if_mixed_branches_001`

#### 既存テスト修正
- `while_expr_value_001.ns`: while 代入を削除、式文として使用に変更
- `if_expr_value_001.ns`: else なし if 代入を削除、式文として使用に変更
- `block_expr_empty_001.ns`: 空ブロック代入を削除、式文として使用に変更
- `e1-00-qsort.ns`: `qsort()` 関数の early return パターンを条件反転+ブロック移動に変更（mixed return 回避）
- `c002.ns`: `__trace()` を算術式/条件式から除去（void 型対応）、trace_hit_counts を再計算
