# エラーテストケース追加計画

各テストケースの詳細設計。ファイル名、ソースコード、check.json の設計を含む。

---

## Phase 1: 字句解析エラー（6 件）

### 1-1. `char_invalid_escape_001` — 未知のエスケープシーケンス

**ファイル**: `resources/tests/fails/syntax/char_invalid_escape_001.ns`

```nospace
# 未知のエスケープシーケンス #
func: main() {
  let: x;
  x = '\q';
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tokenize",
  "contains": ["unknown escape sequence"]
}
```

> カバー: T5 — `unknown escape sequence: \{c}`

---

### 1-2. `char_empty_001` — 空の文字リテラル

**ファイル**: `resources/tests/fails/syntax/char_empty_001.ns`

```nospace
# 空の文字リテラル #
func: main() {
  let: x;
  x = '';
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tokenize",
  "contains": ["empty character literal"]
}
```

> カバー: T7 — `empty character literal`

---

### 1-3. `char_unclosed_001` — 閉じられていない文字リテラル

**ファイル**: `resources/tests/fails/syntax/char_unclosed_001.ns`

```nospace
# 閉じられていない文字リテラル #
func: main() {
  let: x;
  x = 'ab;
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tokenize",
  "contains": ["expected closing quote"]
}
```

> カバー: T8 — `expected closing quote, found: {c}`

---

### 1-4. `char_eof_001` — 文字リテラル中の EOF

**ファイル**: `resources/tests/fails/syntax/char_eof_001.ns`

```nospace
# 文字リテラル中の予期しないEOF #
func: main() {
  let: x;
  x = '
```

**注意**: ファイルはシングルクォートの後で終了する（改行なし or 改行のみ）。
ただし、`'` の直後の改行がどう処理されるかソースの挙動を確認する必要がある。
代替として `'\` (バックスラッシュ直後 EOF) のパターンも検討。

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tokenize",
  "contains": ["character literal"]
}
```

> カバー: T6 — `unexpected end of input in character literal`

---

### 1-5. `string_unclosed_001` — 閉じられていない文字列リテラル

**ファイル**: `resources/tests/fails/syntax/string_unclosed_001.ns`

```nospace
# 閉じられていない文字列リテラル #
func: main() {
  let: a[10] = "hello;
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tokenize",
  "contains": ["string literal"]
}
```

> カバー: T13/T14 — `unclosed string literal` / `unexpected end of input in string literal`

---

### 1-6. `single_pipe_001` — 単独パイプ演算子

**ファイル**: `resources/tests/fails/syntax/single_pipe_001.ns`

```nospace
# 単独パイプ演算子 #
func: main() {
  let: x;
  x = 1 | 2;
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tokenize",
  "contains": ["single '|' is not supported"]
}
```

> カバー: T15 — `single '|' is not supported`

---

## Phase 2: 構文解析エラー（7 件）

### 2-1. `unexpected_comma_001` — 関数呼び出しの不正なカンマ

**ファイル**: `resources/tests/fails/syntax/unexpected_comma_001.ns`

```nospace
# 関数呼び出しの不正なカンマ #
func: foo(a, b) {
  return: a;
}
func: main() {
  let: x;
  x = foo(1,, 2);
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tree",
  "contains": ["unexpected comma"]
}
```

> カバー: P2 — `unexpected comma`

**注意**: 弱いエラー（パース継続）のため、テストフレームワークがこれを error として検出できるか確認が必要。`parse_to_tree` が `Err` を返すことが前提。弱いエラーも含めて `Err` を返すのであれば問題ない。

---

### 2-2. `missing_comma_001` — カンマ欠落

**ファイル**: `resources/tests/fails/syntax/missing_comma_001.ns`

```nospace
# 関数呼び出しのカンマ欠落 #
func: foo(a, b) {
  return: a;
}
func: main() {
  let: x;
  x = foo(1 2);
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tree",
  "contains": ["missing comma"]
}
```

> カバー: P3 — `missing comma`

---

### 2-3. `missing_colon_let_001` — let 文のコロン欠落

**ファイル**: `resources/tests/fails/syntax/missing_colon_let_001.ns`

```nospace
# let 文のコロン欠落 #
func: main() {
  let x;
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tree",
  "contains": ["expected"]
}
```

> カバー: P6 — `unexpected token: expected {pat}` (match_expect_token)

---

### 2-4. `unexpected_factor_001` — 式中の不正なトークン

**ファイル**: `resources/tests/fails/syntax/unexpected_factor_001.ns`

```nospace
# 式中の不正なトークン #
func: main() {
  let: x;
  x = + ;
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tree",
  "contains": ["unexpected"]
}
```

> カバー: P7 — `unexpected token` (因子解析で不正トークン)

---

### 2-5. `array_size_zero_001` — 配列サイズが 0

**ファイル**: `resources/tests/fails/syntax/array_size_zero_001.ns`

```nospace
# 配列サイズが 0 #
func: main() {
  let: a[0];
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tree",
  "contains": ["array size must be positive"]
}
```

> カバー: P8 — `array size must be positive`

---

### 2-6. `array_init_overflow_001` — 配列初期化要素数超過

**ファイル**: `resources/tests/fails/syntax/array_init_overflow_001.ns`

```nospace
# 配列初期化要素数超過 #
func: main() {
  let: a[2] = {1, 2, 3};
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tree",
  "contains": ["too many initializers"]
}
```

> カバー: P11 — `too many initializers for array of size N: got M`

---

### 2-7. `string_too_long_for_array_001` — 文字列リテラルが配列サイズを超過

**ファイル**: `resources/tests/fails/syntax/string_too_long_for_array_001.ns`

```nospace
# 文字列リテラルが配列サイズを超過 #
func: main() {
  let: a[3] = "hello";
}
```

**check.json**:
```json
{
  "type": "parse_error",
  "phase": "tree",
  "contains": ["string literal too long"]
}
```

> カバー: P10 — `string literal too long for array of size N: needs M`

---

## Phase 3: 意味解析エラー（5 件）

### 3-1. `not_an_array_001` — 配列でない変数への添字アクセス

**ファイル**: `resources/tests/fails/compile/not_an_array_001.ns`

```nospace
# 配列でない変数への添字アクセス #
func: main() {
  let: x;
  x = 5;
  __clog(x[0]);
}
```

**check.json**:
```json
{
  "type": "compile_error",
  "contains": ["is not an array"]
}
```

> カバー: S4 — `'{name}' is not an array`

---

### 3-2. `ref_non_variable_001` — 変数/配列要素以外への参照演算子

**ファイル**: `resources/tests/fails/compile/ref_non_variable_001.ns`

```nospace
# 変数・配列要素以外への参照演算子 #
func: main() {
  let: x;
  x = &(1 + 2);
}
```

**check.json**:
```json
{
  "type": "compile_error",
  "contains": ["reference operator"]
}
```

> カバー: S5 — `reference operator (&) can only be applied to variables or array elements`

---

### 3-3. `return_outside_func_001` — 関数外での return

**ファイル**: `resources/tests/fails/compile/return_outside_func_001.ns`

```nospace
# 関数外での return 文 #
return: 0;

func: main() {
  __trace(0);
}
```

**check.json**:
```json
{
  "type": "compile_error",
  "contains": ["return statement outside of function"]
}
```

> カバー: S6 — `semantic error: return statement outside of function`

---

### 3-4. `continue_outside_func_001` — 関数外での continue

**ファイル**: `resources/tests/fails/compile/continue_outside_func_001.ns`

```nospace
# 関数外での continue 文 #
continue;

func: main() {
  __trace(0);
}
```

**check.json**:
```json
{
  "type": "compile_error",
  "contains": ["continue statement outside of function"]
}
```

> カバー: S7 — `semantic error: continue statement outside of function`

---

### 3-5. `break_outside_func_001` — 関数外での break

**ファイル**: `resources/tests/fails/compile/break_outside_func_001.ns`

```nospace
# 関数外での break 文 #
break;

func: main() {
  __trace(0);
}
```

**check.json**:
```json
{
  "type": "compile_error",
  "contains": ["break statement outside of function"]
}
```

> カバー: S8 — `semantic error: break statement outside of function`

---

## Phase 4: 検証とマトリクス更新

全テスト追加後に以下を実施:

1. `cargo test --test code_test` で全テストが成功することを確認
2. `coverage-matrix.md` の状態を ✅ に更新
3. テスト数の集計を更新

---

## test-manifest.yaml への登録

### Phase 1 追加分（Syntax Errors セクション直後に追加）

```yaml
  - name: test_syntax_error_char_invalid_escape_001
    type: syntax_error
    path: char_invalid_escape_001
    comment: "Error: unknown escape sequence in character literal"

  - name: test_syntax_error_char_empty_001
    type: syntax_error
    path: char_empty_001
    comment: "Error: empty character literal"

  - name: test_syntax_error_char_unclosed_001
    type: syntax_error
    path: char_unclosed_001
    comment: "Error: unclosed character literal"

  - name: test_syntax_error_char_eof_001
    type: syntax_error
    path: char_eof_001
    comment: "Error: unexpected EOF in character literal"

  - name: test_syntax_error_string_unclosed_001
    type: syntax_error
    path: string_unclosed_001
    comment: "Error: unclosed string literal"

  - name: test_syntax_error_single_pipe_001
    type: syntax_error
    path: single_pipe_001
    comment: "Error: single pipe operator is not supported"
```

### Phase 2 追加分

```yaml
  - name: test_syntax_error_unexpected_comma_001
    type: syntax_error
    path: unexpected_comma_001
    comment: "Error: unexpected comma in function call"

  - name: test_syntax_error_missing_comma_001
    type: syntax_error
    path: missing_comma_001
    comment: "Error: missing comma in function call"

  - name: test_syntax_error_missing_colon_let_001
    type: syntax_error
    path: missing_colon_let_001
    comment: "Error: missing colon after let keyword"

  - name: test_syntax_error_unexpected_factor_001
    type: syntax_error
    path: unexpected_factor_001
    comment: "Error: unexpected token in expression"

  - name: test_syntax_error_array_size_zero_001
    type: syntax_error
    path: array_size_zero_001
    comment: "Error: array size must be positive"

  - name: test_syntax_error_array_init_overflow_001
    type: syntax_error
    path: array_init_overflow_001
    comment: "Error: too many initializers for array"

  - name: test_syntax_error_string_too_long_for_array_001
    type: syntax_error
    path: string_too_long_for_array_001
    comment: "Error: string literal too long for array"
```

### Phase 3 追加分

```yaml
  - name: test_compile_error_not_an_array_001
    type: compile_error
    path: not_an_array_001
    comment: "Error: subscript access on non-array variable"

  - name: test_compile_error_ref_non_variable_001
    type: compile_error
    path: ref_non_variable_001
    comment: "Error: reference operator on non-variable"

  - name: test_compile_error_return_outside_func_001
    type: compile_error
    path: return_outside_func_001
    comment: "Error: return statement outside of function"

  - name: test_compile_error_continue_outside_func_001
    type: compile_error
    path: continue_outside_func_001
    comment: "Error: continue statement outside of function"

  - name: test_compile_error_break_outside_func_001
    type: compile_error
    path: break_outside_func_001
    comment: "Error: break statement outside of function"
```

---

## 実装上の注意事項

### syntax_error テストの `contains` チェック

現在の `test_syntax_error_base` 関数は `contains` フィールドを読み取るものの、**実際のエラーメッセージとの照合を行っていない**（`error_count` と `contains` は `_` で無視されている）。

```rust
// tests/code_test.rs L244-L246
TestConfig::ParseError {
    phase,
    error_count: _,
    contains: _,
} => ...
```

テスト追加と合わせて、`contains` チェックを有効化することを推奨する。ただし、これは既存テストに影響する可能性があるため、別ステップとして慎重に行う。

### 弱いエラーの検出

Tree parser の「弱いエラー」（`unexpected comma`, `missing comma` 等）は、パースを継続するためエラーリストに追加されるが、`parse_to_tree()` が `Err` を返すことで検出可能。ただし、弱いエラーのみの場合に `Err` を返すかどうかは実装を確認する必要がある。

### char_eof_001 のファイル末尾

ファイル末尾がシングルクォートで終わるケースでは、OS やエディタが自動的に末尾改行を追加する可能性がある。テスト作成時にはファイル内容を慎重に管理する必要がある。
