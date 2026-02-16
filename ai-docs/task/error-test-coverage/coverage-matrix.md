# エラーテストカバレッジマトリクス

ソースコードから抽出した全エラーパスと、既存テストケースの対応表。

凡例:
- ✅ テストケースあり
- ❌ テストケースなし（追加予定）
- ➖ 対象外（内部エラー・panic 等）
- 🔲 対象外（別タスクで検討）

---

## 1. 字句解析エラー (token_parser)

テスト種別: `syntax_error` / `phase: "tokenize"`

| # | エラーメッセージ | テストケース | 状態 |
|---|---|---|---|
| T1 | `invalid hexadecimal literal: expected at least one hex digit after '0x'` | `hex_invalid_001` | ✅ |
| T2 | `incomplete hex escape sequence: expected 2 hex digits after '\x'` (char, 1桁目) | `char_hex_invalid_001` | ✅ |
| T3 | `incomplete hex escape sequence: expected 2 hex digits after '\x'` (char, 2桁目) | `char_hex_invalid_002` | ✅ |
| T4 | `invalid hex escape sequence: \x{HH}` | — | ❌ |
| T5 | `unknown escape sequence: \{c}` | — | ❌ |
| T6 | `unexpected end of input in character literal` | — | ❌ |
| T7 | `empty character literal` | — | ❌ |
| T8 | `expected closing quote, found: {c}` / `unclosed character literal` | — | ❌ |
| T9 | `incomplete hex escape sequence: ...` (string literal, 1桁目) | — | ❌ |
| T10 | `incomplete hex escape sequence: ...` (string literal, 2桁目) | — | ❌ (T2 と同類) |
| T11 | `invalid hex escape sequence: ...` (string literal) | — | ❌ (T4 と同類) |
| T12 | `unknown escape sequence: ...` (string literal) | — | ❌ (T5 と同類) |
| T13 | `unexpected end of input in string literal` | — | ❌ |
| T14 | `unclosed string literal: expected closing '"'` | — | ❌ (T13 と同類) |
| T15 | `single '\|' is not supported` | — | ❌ |
| T16 | `invalid char: {c}` | `invalid_token_001` | ✅ |
| T17 | `failed to convert to token: ...` | — | ➖ (内部エラー) |
| T18 | `panic!("internal error")` (parse_identifier) | — | ➖ |

### 追加するテスト: 6 件

文字リテラルと文字列リテラルで類似エラーがあるが、文字リテラル側で代表テストを作成する。文字列側は実装パスが異なるため別途確認する価値があるが、Phase 1 では主要なパスのみカバーする。

---

## 2. 構文解析エラー (tree_parser)

テスト種別: `syntax_error` / `phase: "tree"`

| # | エラーメッセージ | テストケース | 状態 |
|---|---|---|---|
| P1 | `unexpected end of input` (式途中 EOF) | `unexpected_eof_001` | ✅ |
| P2 | `unexpected comma` | — | ❌ |
| P3 | `missing comma` | — | ❌ |
| P4 | 括弧が閉じられていない (unclosed paren) | `unclosed_paren_001` | ✅ |
| P5 | `unexpected token (unmatched closing brace or extra code)` | `only_closing_brace_001`, `extra_closing_braces_001` | ✅ |
| P6 | `unexpected token: expected {pat}` (match_expect_token) | — | ❌ |
| P7 | `unexpected token` (因子解析で不正トークン) | — | ❌ |
| P8 | `array size must be positive` | — | ❌ |
| P9 | `expected array size` | — | ❌ |
| P10 | `string literal too long for array of size N: needs M` | — | ❌ |
| P11 | `too many initializers for array of size N: got M` | — | ❌ |
| P12 | `expected ','` (関数引数定義リスト) | — | ❌ |
| P13 | `unexpected ','` (関数引数定義リスト) | — | ❌ (P2 と同類) |

### 追加するテスト: 7 件

配列関連のエラー（P8-P11）と、関数定義引数リスト関連のエラー（P12-P13）は優先度がやや低いが、Phase 2 で対応する。

---

## 3. 意味解析エラー (semantic_analyzer)

テスト種別: `compile_error`

| # | エラーメッセージ | テストケース | 状態 |
|---|---|---|---|
| S1 | `undefined variable: {name}` | `scope_undefined_001` | ✅ |
| S2 | `semantic error: the name '{name}' is already used` | `scope_duplicate_001`, `global_duplicate_001`, `func_duplicate_*`, `func_var_conflict_001` | ✅ |
| S3 | `undefined function: {f}` | `scope_nested_func_child_access_error_001` | ✅ |
| S4 | `'{name}' is not an array` | — | ❌ |
| S5 | `reference operator (&) can only be applied to variables or array elements` | — | ❌ |
| S6 | `semantic error: return statement outside of function` | — | ❌ |
| S7 | `semantic error: continue statement outside of function` | — | ❌ |
| S8 | `semantic error: break statement outside of function` | — | ❌ |
| S9 | スコープ外変数参照 | `scope_out_of_scope_001` | ✅ |
| S10 | 非 static 変数の関数境界越えアクセス | `scope_static_error_001` | ✅ |

### 追加するテスト: 5 件

S6-S8 は現在のプログラム構造上ではトップレベルの return/continue/break が必要。意味解析のチェック対象であるため、テストを追加する。

---

## 4. コンパイルエラー (compiler_ws)

テスト種別: `compile_error`

| # | エラーメッセージ | テストケース | 状態 |
|---|---|---|---|
| C1 | `main function not found` (MainNotFound) | `no_main_001` | ✅ |
| C2 | `Undefined variable: {name}` (UndefinedVariable) | — | ➖ (意味解析でキャッチ) |
| C3 | `Undefined function: {name}` (UndefinedFunction) | — | ➖ (意味解析でキャッチ) |
| C4 | `Invalid operation: ...` 各種 | — | ➖ (意味解析通過後のみ発生、特殊ケース) |

### 追加するテスト: 0 件

コンパイルエラーは意味解析でほぼ全てキャッチされるため、追加テストの優先度は低い。

---

## 5. 統計サマリ

| フェーズ | 既存テスト | 追加予定 | カバー率（追加後） |
|---------|-----------|---------|-----------------|
| 字句解析 | 4 / 16 (25%) | +6 | 10 / 16 (63%) |
| 構文解析 | 4 / 13 (31%) | +7 | 11 / 13 (85%) |
| 意味解析 | 5 / 10 (50%) | +5 | 10 / 10 (100%) |
| コンパイル | 1 / 4 (25%) | +0 | 1 / 4 (25%) |
| **合計** | **14 / 43** | **+18** | **32 / 43 (74%)** |

※ 内部エラー（➖）を除いた計算
