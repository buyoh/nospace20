# 配列初期化構文の仕様変更

## 概要

配列初期化の構文を変更し、初期化リストを `[]` で囲む形式にする。また、初期値から配列サイズを推論できるようにする。

## 背景

現在の配列初期化構文は `let: arr[3](10, 20, 30);` のように `()` で値リストを囲む形式だが、これは関数呼び出しと視覚的に区別しにくい。また、配列サイズを明示的に指定する必要があり、初期値の個数と一致するかチェックする必要がある。

## 仕様変更

### 変更前

```nospace
let: arr[3](10, 20, 30);          # 初期値付き配列宣言 #
let: str[10]("Hello");            # 文字列初期化 #
```

### 変更後

```nospace
let: arr[3]([10, 20, 30]);        # 初期値付き配列宣言 #
let: arr[]([10, 20, 30]);         # サイズ省略（初期値から推論）#
let: str[]("Hello");              # 文字列もサイズ省略可能 #
let: str[10]("Hello");            # サイズ明示も可能 #
```

**変更点:**

1. **初期化リストを `[]` で囲む**: `(10, 20, 30)` → `([10, 20, 30])`
   - 配列リテラル風の記法で視覚的に配列とわかりやすい
   
2. **配列サイズの省略**: `let: arr[](init);` の形式を許可
   - `[]` が指定されている場合、初期値から配列サイズを推論
   - 数値リストの場合: 要素数がそのまま配列サイズ
   - 文字列の場合: 文字数 + 1（ヌル終端分）

3. **エラーケース**:
   - `[]` でサイズ省略しているのに初期値がない: `let: arr[];` → エラー
   - 初期値リストが空: `let: arr[]([]);` → エラー（サイズ0の配列は不正）
   - サイズ明示と初期値の個数が不一致: `let: arr[2]([1, 2, 3]);` → エラー

## 構文（BNF）

### 変更前

```bnf
let_decl ::= ident ("[" integer "]")? ("(" expr ("," expr)* ")")?
```

### 変更後

```bnf
let_decl ::= ident ("[" integer? "]")? ("(" array_init | string_init | expr ")")?
array_init ::= "[" expr ("," expr)* "]"
string_init ::= string_literal
```

**詳細:**

- `ident[N]([...])` - サイズN、初期値あり
- `ident[]([...])` - サイズ推論、初期値あり
- `ident[N]("...")` - サイズN、文字列初期化
- `ident[]("...")` - サイズ推論、文字列初期化
- `ident[N]` - サイズN、初期値なし（デフォルト0）
- `ident(expr)` - 通常変数の初期化（従来通り）

## 影響範囲

### 1. 仕様書・ドキュメント

#### spec.md

- **§4.2 配列**: 構文例を更新
- **§4.3 文字列**: 構文例を更新

現在:
```nospace
let: arr[4];                   # サイズ4の配列を宣言 #
arr[0] = 1;                    # 要素へのアクセス #
let: arr2[3](10, 20, 30);      # 初期値付き配列宣言 #
```

変更後:
```nospace
let: arr[4];                   # サイズ4の配列を宣言 #
arr[0] = 1;                    # 要素へのアクセス #
let: arr2[3]([10, 20, 30]);    # 初期値付き配列宣言 #
let: arr3[]([10, 20, 30]);     # サイズ省略（3と推論）#
```

#### tutorial.md

- **Sorting (Quick Sort)** の例: `let: arr[9](3,1,4,1,5,9,2,6,5);`
  → `let: arr[9]([3,1,4,1,5,9,2,6,5]);` または `let: arr[]([3,1,4,1,5,9,2,6,5]);`

#### docs/grammar.bnf

- `let_decl` の定義を更新

### 2. テストケース

以下のテストファイルが影響を受ける:

#### Passes（成功するテスト）

| ファイル | 該当行 | 変更前 | 変更後 |
|---------|-------|--------|--------|
| `array-basic.ns` | 13 | `let: arr2[3](100, 200, 300);` | `let: arr2[3]([100, 200, 300]);` |
| `array-reference.ns` | 19 | `let: arr2[3](1, 2, 3);` | `let: arr2[3]([1, 2, 3]);` |
| `string-basic.ns` | 30 | `let: s3[10]("Hi");` | `let: s3[10]("Hi");` (変更なし) |
| `legacy/legacy_021.ns` | 1 | `let:g[2](2,3);` | `let:g[2]([2,3]);` |
| `legacy/legacy_022.ns` | 13 | `let:x[5](1,2,3,4,5);` | `let:x[5]([1,2,3,4,5]);` |
| `legacy/legacy_024.ns` | 10, 15 | `let:b[3](3,4,5);` | `let:b[3]([3,4,5]);` |
| `examples/e0-00-puts.ns` | 10 | `let: g[12]("hello\sworld");` | `let: g[12]("hello\sworld");` (変更なし) |

**注**: 文字列初期化 `("...")` は `[]` で囲まず、そのまま `("...")` とする（文字列リテラルは既に配列的なオブジェクトなため）。

#### Fails（失敗するテスト）

| ファイル | 該当行 | 変更前 | 変更後 | 備考 |
|---------|-------|--------|--------|------|
| `fails/syntax/array_init_overflow_001.ns` | 3 | `let: a[2] = {1, 2, 3};` | `let: a[2]([1, 2, 3]);` | エラーメッセージは維持 |
| `fails/syntax/string_too_long_for_array_001.ns` | 3 | `let: a[3] = "hello";` | `let: a[3]("hello");` | エラーメッセージは維持 |

**注**: fails系のテストは現在 `= {...}` 構文を使っているが、これは未実装の構文。新しい構文に合わせて修正する。

#### 新規テストケース

以下のテストケースを追加すべき:

1. **サイズ省略 - 数値リスト**
   ```nospace
   let: arr[]([1, 2, 3]);
   # arr[0]==1, arr[1]==2, arr[2]==3 を確認 #
   ```

2. **サイズ省略 - 文字列**
   ```nospace
   let: str[]("ABC");
   # str[0]=='A', str[1]=='B', str[2]=='C', str[3]==0 を確認 #
   ```

3. **エラー: サイズ省略で初期値なし**
   ```nospace
   let: arr[];  # エラー: サイズ未定 #
   ```

4. **エラー: 空の初期化リスト**
   ```nospace
   let: arr[]([]);  # エラー: サイズ0は不正 #
   ```

### 3. ソースコード

#### src/tree_parser/statement/mod.rs

`parse_variable_declarations` 関数を修正:

**変更点:**

1. **配列サイズの扱い**:
   - 現在: `[N]` でサイズ取得 → `array_size = Some(N)`
   - 変更後: `[N]` → `array_size = Some(N)`, `[]` → `array_size = None` (一時的)
   - 初期化後にサイズ推論して確定

2. **初期化リストのパース**:
   - 現在: `(val1, val2, val3)` → カンマ区切りで読み取り
   - 変更後: `([val1, val2, val3])` → `(` の後に `[` を期待、カンマ区切りで読み取り、`]` `)`

3. **サイズ推論ロジック**:
   ```rust
   // 疑似コード
   if bracket_specified && array_size.is_none() {
       // [] の場合、初期値から推論
       if 文字列初期化 {
           array_size = Some(string.len() + 1);
       } else if 数値リスト初期化 {
           array_size = Some(init_values.len());
       } else {
           エラー: サイズ不明
       }
   }
   ```

4. **エラーチェック**:
   - `[]` でサイズ省略しているのに初期値がない → エラー
   - 初期化リストが空（`[]`内に要素がない）→ エラー
   - サイズ明示されている場合の個数チェック（従来通り）

**詳細設計:**

```rust
fn parse_variable_declarations(...) -> Vec<LocatedStatement> {
    loop {
        let id = /* 識別子取得 */;
        
        // 配列サイズのパース
        let bracket_specified = false;
        let array_size = if peek() == BracketL {
            next(); // '[' 消費
            bracket_specified = true;
            
            if peek() == BracketR {
                // "[]" - サイズ省略
                next(); // ']' 消費
                None  // 後で推論
            } else if peek() == Number(n) {
                // "[N]" - サイズ指定
                next(); // Number 消費
                expect(BracketR); // ']' 消費
                Some(n)
            } else {
                エラー: expected number or ']'
            }
        } else {
            None  // 通常変数
        };
        
        // 初期化式のパース
        if peek() == ParenthesisL {
            next(); // '(' 消費
            
            if bracket_specified {
                // 配列の初期化
                
                if peek() == StringLiteral {
                    // 文字列初期化: ("Hello")
                    let chars = /* StringLiteral取得 */;
                    expect(ParenthesisR);
                    
                    // サイズ推論
                    let inferred_size = chars.len() + 1;
                    if let Some(explicit_size) = array_size {
                        // サイズチェック
                        if inferred_size > explicit_size {
                            エラー: string too long
                        }
                    } else {
                        array_size = Some(inferred_size);
                    }
                    
                    // 配列宣言 + 初期化文を生成
                    ...
                    
                } else if peek() == BracketL {
                    // 数値リスト初期化: ([1, 2, 3])
                    next(); // '[' 消費
                    
                    let mut init_values = vec![];
                    loop {
                        if peek() == BracketR {
                            break;
                        }
                        let expr = parse_expression();
                        init_values.push(expr);
                        
                        if peek() == Comma {
                            next();
                        } else {
                            break;
                        }
                    }
                    
                    expect(BracketR); // ']' 消費
                    expect(ParenthesisR); // ')' 消費
                    
                    // サイズチェック・推論
                    if init_values.is_empty() {
                        エラー: empty initializer list
                    }
                    
                    let inferred_size = init_values.len() as i64;
                    if let Some(explicit_size) = array_size {
                        if init_values.len() > explicit_size {
                            エラー: too many initializers
                        }
                    } else {
                        array_size = Some(inferred_size);
                    }
                    
                    // 配列宣言 + 初期化文を生成
                    ...
                    
                } else {
                    エラー: expected '[' or string literal
                }
                
            } else {
                // 通常変数の初期化: (expr)
                let expr = parse_expression();
                expect(ParenthesisR);
                ...
            }
        } else {
            // 初期化式なし
            if bracket_specified && array_size.is_none() {
                エラー: array size not specified and no initializer
            }
        }
        
        ...
    }
}
```

#### src/tree_parser/statement/test.rs

ユニットテストを更新:

1. `test_parse_array_declaration_with_init`: トークン列を更新
   - 追加: `BracketL`, `BracketR` トークン
   
2. 新規テスト追加:
   - `test_parse_array_declaration_size_omitted_with_init`
   - `test_parse_array_declaration_size_omitted_string`
   - `test_parse_array_declaration_size_omitted_no_init_error`
   - `test_parse_array_declaration_empty_init_error`

## 実装手順

### Phase 1: 仕様書・BNFの更新

1. `spec.md` の §4.2, §4.3 を更新
2. `tutorial.md` の例を更新
3. `docs/grammar.bnf` を更新

### Phase 2: パーサーの実装

1. `src/tree_parser/statement/mod.rs` の `parse_variable_declarations` を修正
   - `[]` のサポート（サイズ省略）
   - `([...])` 初期化リストのサポート
   - サイズ推論ロジック
   - エラーチェック

2. ユニットテストの更新・追加
   - `src/tree_parser/statement/test.rs`

### Phase 3: テストケースの更新

1. 既存テストの修正（構文を新しい形式に）
   - `resources/tests/passes/array-basic.ns`
   - `resources/tests/passes/array-reference.ns`
   - `resources/tests/passes/legacy/legacy_021.ns`
   - `resources/tests/passes/legacy/legacy_022.ns`
   - `resources/tests/passes/legacy/legacy_024.ns`
   - `resources/tests/fails/syntax/array_init_overflow_001.ns`
   - `resources/tests/fails/syntax/string_too_long_for_array_001.ns`

2. 新規テストの追加
   - サイズ省略のテスト
   - エラーケースのテスト

### Phase 4: 統合テスト

1. すべてのテストが通ることを確認
   - `cargo test`
   - `./tools/ci/run-tests.sh` (if exists)

2. エラーメッセージの確認

## 注意点・考慮事項

### 1. 後方互換性

この変更は **破壊的変更** である。既存のコードは新しい構文に書き換える必要がある。

### 2. 文字列初期化の特殊扱い

文字列初期化は `("Hello")` の形式で、`[]` で囲まない。理由:

- 文字列リテラル自体が既に配列的なオブジェクト
- `(["Hello"])` は不自然
- `("Hello")` は直感的

### 3. 空配列の扱い

`let: arr[]([]);` （空の初期化リスト）はエラーとする。理由:

- 配列サイズ0は現在の仕様で禁止されている
- サイズが不明確

### 4. 混在しない

配列初期化では `([...])` と `("...")` のどちらか一方のみを使用。混在は不可:

```nospace
# 不正: #
let: mixed[]([1, 2, "hello"]);
```

### 5. 多次元配列

現在未実装のため考慮不要。将来的には:

```nospace
let: matrix[][]([[1, 2], [3, 4]]);  # 未実装 #
```

のような構文を検討する可能性がある。

## 移行ガイド

既存コードの移行方法:

### 自動変換

以下の正規表現で一括置換可能（簡易版）:

```regex
# 数値リスト初期化
\[(\d+)\]\(([0-9, ]+)\)
→
[$1]([$2])

# 文字列初期化（変更なし）
\[(\d+)\](".*?")
→
[$1]($2)
```

**注**: 式を含む初期化は正規表現では対応困難。手動で確認を推奨。

### サンプル移行

**移行前:**
```nospace
let: arr[3](10, 20, 30);
let: str[10]("Hello");
```

**移行後（サイズ維持）:**
```nospace
let: arr[3]([10, 20, 30]);
let: str[10]("Hello");
```

**移行後（サイズ省略）:**
```nospace
let: arr[]([10, 20, 30]);
let: str[]("Hello");
```

## 実装後の検証項目

- [x] すべてのテストがパスする
- [x] エラーメッセージが適切
- [x] spec.md が更新されている
- [x] tutorial.md が更新されている
- [x] grammar.bnf が更新されている
- [x] サイズ省略の動作確認
- [x] エラーケースの動作確認

## 実装結果

### 実装日: 2026-02-24

**完了した作業:**

1. **仕様書・BNFの更新**
   - `spec.md` §4.2（配列）・§4.3（文字列）を更新、TODOセクションを削除
   - `tutorial.md` のサンプルコードを更新（`let: arr[]([3,1,4,1,5,9,2,6,5])`）
   - `docs/grammar.bnf` の `let_decl` 定義を更新、`array_init`・`string_init` 定義を追加

2. **パーサーの実装** (`src/tree_parser/statement/mod.rs`)
   - `parse_variable_declarations` 関数を修正
   - `bracket_specified` フラグを導入し `[]` と `[N]` を区別
   - `([v1, v2, ...])` 形式の数値リスト初期化をサポート
   - `[]` でサイズ省略時のサイズ推論ロジックを実装
   - エラーチェック追加: サイズ省略で初期値なし、空の初期化リスト

3. **ユニットテスト** (`src/tree_parser/statement/test.rs`)
   - `test_parse_array_declaration_with_init`: 新構文 `([...])` に更新
   - 新規追加: `test_parse_array_declaration_size_omitted_with_init`
   - 新規追加: `test_parse_array_declaration_size_omitted_string`
   - 新規追加: `test_parse_array_declaration_size_omitted_no_init_error`
   - 新規追加: `test_parse_array_declaration_empty_init_error`

4. **既存テストの修正**
   - `passes/array-basic.ns`: `(100, 200, 300)` → `([100, 200, 300])`
   - `passes/array-reference.ns`: `(1, 2, 3)` → `([1, 2, 3])`
   - `passes/legacy/legacy_021.ns`: `(2,3)` → `([2,3])`
   - `passes/legacy/legacy_022.ns`: `(1,2,3,4,5)` → `([1,2,3,4,5])`
   - `passes/legacy/legacy_024.ns`: `(3,4,5)` → `([3,4,5])`, `(0,1,2)` → `([0,1,2])`
   - `passes/string-basic.ns`: `let: s("Hello")` → `let: s[]("Hello")` 等
   - `passes/string-escape.ns`: `let: esc(...)` → `let: esc[](...)` 等
   - `passes/array-static.ns`: `(10, 20)` → `([10, 20])`
   - `fails/syntax/array_init_overflow_001.ns`: `= {1, 2, 3}` → `([1, 2, 3])`
   - `fails/syntax/string_too_long_for_array_001.ns`: `= "hello"` → `("hello")`

5. **新規テストケースの追加**
   - `passes/array-size-omitted-list.ns`: サイズ省略 + 数値リスト初期化
   - `passes/array-size-omitted-string.ns`: サイズ省略 + 文字列初期化
   - `fails/syntax/array_size_omitted_no_init_001.ns`: エラー: サイズ省略で初期値なし
   - `fails/syntax/array_empty_init_list_001.ns`: エラー: 空の初期化リスト
   - `test-manifest.yaml` にすべて登録済み

**テスト結果**: 632 passed; 0 failed; 126 ignored
