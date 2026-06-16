# Phase コメント整理タスク

## 概要

ソースコード（`.rs`）中に `//! Phase N: ...` / `/// Phase N: ...` の形式で残っている開発メモを整理する。
これらは過去の実装フェーズ（scope Phase 1-6、expression-location Phase 1-2、symbol-table-impl Phase 5-6 など）を示すコメントであり、該当タスクはすべて `done-task/` にアーカイブ済みである。

### 関連する完了済みタスク

| 参照フェーズ | タスク | 完了日 |
|---|---|---|
| Phase 1: コンパイルエラー位置情報 | `done-task/expression-location/` | 2026-02-26 |
| Phase 2: 識別子事前解決 | `done-task/phase2-identifier-resolution.md` | 2026-02-11 |
| Phase 4: static 変数永続化 | `done-task/scope-phase4-static-variables.md` | 2026-02-11 |
| Phase 5: ネスト関数・スコープ制御 | `done-task/scope/` | 2026-02-11 |
| Phase 6: インデックスベース関数参照 | `done-task/symbol-table-impl/` | 2026-02-17 |
| Phase 7: コンパイラ関数生成 | （compiler_ws 実装済、タスク名なし） | — |

## 削除対象コメント一覧

### カテゴリ A: 行ごと完全削除（意味のある情報を含まない）

| ファイル | 行 | コメント本文 | 理由 |
|---|---|---|---|
| `tests/compile_test.rs` | 70 | `/// Phase 1 実装後、エラーに文レベルの位置情報が含まれることを確認する。` | 将来形の計画コメント。Phase 1 は実装済み |
| `tests/compile_test.rs` | 38 | `/// (Phase 1: コンパイルエラーの位置情報サポート)` | テストの意図はテスト名で明確 |
| `tests/compile_test.rs` | 91 | `// Phase 1: 文レベルの位置情報が含まれること` | assert が直後にあり冗長 |
| `src/semantic_analyzer/scope.rs` | 172 | `/// Phase 5 で追加` | フィールド説明が他にない場合のみ注記として残しても良いが、説明は親の doc comment にある |
| `src/semantic_analyzer/tests.rs` | 91 | `// Phase 5: ネスト関数がサポートされたため、以下のテストは削除` | 削除済みテストへの注釈 |
| `src/semantic_analyzer/tests.rs` | 93 | `// Phase 5 でネスト関数が正式にサポートされたため、このテストは不要になった` | 同上 |

### カテゴリ B: 「Phase N:」プレフィックスのみ削除（説明本文は保持）

コメント本文の情報は有用なので、`Phase N:` / `Phase N で追加：` / `（Phase N で実装）` の部分だけ除去し、残りの説明を維持する。

#### `src/base/error/compile_error.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 35 | `/// Phase 1: 文の開始位置。式レベルのエラーは文の位置で代替。` | `/// 文の開始位置。式レベルのエラーは文の位置で代替。` |

#### `src/compiler_ws/expression.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 15 | `/// Phase 1: 式レベルのエラーは直近の文の開始位置で代替表示。` | `/// 式レベルのエラーは直近の文の開始位置で代替表示。` |
| 54 | `// Phase 6: BuiltinFunctionKind enum を使用` | `// BuiltinFunctionKind enum を使用` |
| 57 | `// Phase 5: ユーザー定義関数呼び出し` | `// ユーザー定義関数呼び出し` |

#### `src/compiler_ws/statement.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 51 | `// ④ Phase 7: 全ての関数定義を生成` | `// ④ 全ての関数定義を生成` |

#### `src/semantic_analyzer/mod.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 249 | `// Phase 5: identifier_map も保持して関数解決に使用` | `// identifier_map も保持して関数解決に使用` |
| 251 | `// Phase 5: 関数解決に必要` (inline) | `// 関数解決に必要` |
| 261 | `// Phase 6: 一時スコープなので None` (inline) | `// 一時スコープなので None` |

#### `src/semantic_analyzer/scope.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 120 | `/// Phase 5: 関数リストを pub(crate) に変更（interpreter からアクセスするため）` | `/// 関数リスト（interpreter からアクセスするため pub(crate)）` |
| 127 | `/// Phase 6: 関数名による検索を排除し、インデックスベースでアクセス` | `/// 関数名による検索を排除し、インデックスベースでアクセス` |
| 162 | `/// Phase 5: 関数の可視性チェックのため、関数マップも保持。` | `/// 関数の可視性チェックのため、関数マップも保持。` |
| 327 | `/// Phase 5 で追加：ネスト関数の可視性チェック` | `/// ネスト関数の可視性チェック` |
| 328 | `/// Phase 5 修正：全関数はグローバルに格納されるため、常に is_global=true を返す` | `/// 全関数はグローバルに格納されるため、常に is_global=true を返す` |
| 338 | `// Phase 5: 全関数はルートスコープにフラット化されているため、` | `// 全関数はルートスコープにフラット化されているため、` |
| 424 | `/// Phase 5: functions と function_names を削除（グローバル管理に移行）` | `/// functions と function_names は引数で渡す（グローバル管理）` |
| 445 | `/// Phase 5: functions と function_names を引数として受け取る` | `/// functions と function_names を引数として受け取る` |
| 468 | `// Phase 6: __main 関数のインデックスを解決` | `// __main 関数のインデックスを解決` |
| 471 | `// Phase 6: SymbolTable を構築` | `// SymbolTable を構築` |

#### `src/semantic_analyzer/expression.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 283 | `// Phase 5: 組み込み関数とユーザー定義関数を区別` | `// 組み込み関数とユーザー定義関数を区別` |
| 297 | `// Phase 6: 文字列を BuiltinFunctionKind に変換` | `// 文字列を BuiltinFunctionKind に変換` |

#### `src/semantic_analyzer/statement.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 101 | `// Phase 5: パス1aで登録済みの関数のグローバルインデックスを取得` | `// パス1aで登録済みの関数のグローバルインデックスを取得` |
| 110 | `// Phase 5: global_functions と global_function_names を渡す` | `// global_functions と global_function_names を渡す` |
| 125 | `// Phase 5: 非ルートスコープの build() には空の functions/function_names を渡す` | `// 非ルートスコープの build() には空の functions/function_names を渡す` |

#### `src/semantic_analyzer/types.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 136 | `///   関数も IdentifierRef を使用し、スコープ解決を行う（Phase 5 で実装）。` | `///   関数も IdentifierRef を使用し、スコープ解決を行う。` |
| 149 | `/// Phase 6: 組み込み関数は BuiltinFunctionKind enum で識別` | `/// 組み込み関数は BuiltinFunctionKind enum で識別` |
| 152 | `/// Phase 5 で追加：スコープ解決済みの関数参照を保持` | `/// スコープ解決済みの関数参照を保持` |

#### `src/interpreter/exec.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 241 | `/// Phase 5: IdentifierRef を使用してユーザー定義関数を呼び出す` | `/// IdentifierRef を使用してユーザー定義関数を呼び出す` |
| 254 | `// Phase 5: 全関数は root_scope にフラット化されているため、` | `// 全関数は root_scope にフラット化されているため、` |
| 258 | `// Phase 4: static 変数の永続化対応` | `// static 変数の永続化対応` |
| 296 | `// Phase 4: static 変数の値を永続ストレージに保存` | `// static 変数の値を永続ストレージに保存` |
| 554 | `// Phase 2: IdentifierRef を使用して O(1) でアクセス` | `// IdentifierRef を使用して O(1) でアクセス` |
| 617 | `// Phase 5: BuiltinFunction と UserFunction に分離` | `// BuiltinFunction と UserFunction に分離` |
| 618 | `// Phase 6: BuiltinFunction は BuiltinFunctionKind enum を使用` | `// BuiltinFunction は BuiltinFunctionKind enum を使用` |

#### `src/interpreter/mod.rs`

| 行 | 削除前 | 削除後 |
|---|---|---|
| 100 | `// Phase 6: インデックスベースで関数にアクセス` | `// インデックスベースで関数にアクセス` |
| 138 | `// Phase 6: 関数インデックスをキーとして使用` | `// 関数インデックスをキーとして使用` |
| 148 | `// Phase 6: main_function_index を使用してインデックスベースでアクセス` | `// main_function_index を使用してインデックスベースでアクセス` |

## 対象外（将来予定の Phase コメント）

以下は **未実装タスクの設計ドキュメント内** にある Phase 参照であり、削除しない。

- `docs-ai/task/temporary-array-literals.md` 内の Phase 1-3（未着手タスク）
- `docs-ai/task/multi-error-reporting.md` 内の各 Phase（進行中タスク）
- `docs-ai/task/` 配下のその他ドキュメント内 Phase 参照

## 作業手順

1. カテゴリ A の 6 行を完全削除（テスト関数の doc comment 調整を含む）
2. カテゴリ B の 32 箇所から `Phase N:` / `Phase N で追加：` / `（Phase N で実装）` を除去
3. `cargo test` でテストが全て通ることを確認
4. コミット（対象ファイルのみ）

## 進捗

- [ ] カテゴリ A: 完全削除 6 件
- [ ] カテゴリ B: Phase プレフィックス削除 32 件（`tests/`, `src/` 計 9 ファイル）
- [ ] `cargo test` 確認
- [ ] コミット
