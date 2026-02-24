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
