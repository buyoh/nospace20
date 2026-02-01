# 新機能実装タスク

## 概要

以下の未実装機能を設計・実装・評価する:
1. 文字リテラル (spec 1.3)
2. 論理演算子 (spec 2.4)
3. 剰余演算子 (spec 2.6)
4. 標準入出力 (spec 3.2) - ※既に実装済み、確認のみ

## 1. 文字リテラル

### 仕様 (spec.md 1.3)

単一の文字をシングルクォートで囲むと、そのASCII値（整数）として扱われる。

```
'A'      # 65 #
'0'      # 48 #
'\n'     # 10 (改行) #
'\t'     # 9 (タブ) #
'\s'     # 32 (スペース) #
'\\'     # 92 (バックスラッシュ) #
'\''     # 39 (シングルクォート) #
```

### 設計方針

**影響範囲**: token_parser のみ

**変更内容**:
1. `Token` 列挙型に変更は不要（`Token::Number(i64)` として表現可能）
2. `parse_to_tokens_internal` 関数で `'` を検出した場合、文字リテラルをパースする
3. エスケープシーケンスの処理を実装

**実装手順**:
1. `src/token_parser/mod.rs` に `parse_char_literal` 関数を追加
2. メインパースループで `'` を検出した際にこの関数を呼び出す
3. エスケープシーケンス `\n`, `\r`, `\t`, `\s`, `\\`, `\'` を処理

**エラーハンドリング**:
- 閉じ `'` が見つからない場合
- 未知のエスケープシーケンスの場合
- 空の文字リテラル `''` の場合

---

## 2. 論理演算子

### 仕様 (spec.md 2.4)

```
1 && 1   # 論理AND: 両方が非0なら 1、そうでなければ 0 #
1 || 0   # 論理OR: どちらかが非0なら 1、両方が0なら 0 #
!0       # 論理NOT: 0なら 1、非0なら 0 #
```

- `&&` : 短絡評価。左辺が 0 なら右辺を評価せず 0 を返す
- `||` : 短絡評価。左辺が非0なら右辺を評価せずその値を返す (※仕様曖昧: 1を返すかその値を返すか)
- `!` : 単項演算子。0 なら 1、非0 なら 0

### 設計方針

**影響範囲**: token_parser, tree_parser, semantic_analyzer, interpreter

**変更内容**:

#### token_parser
1. `Token` 列挙型に `DoubleAmpersand`, `DoublePipe` を追加
2. `parse_to_tokens_internal` で `&&` と `||` をパース
3. `Token::Exclamation` は既に存在（単項 `!` として使用）

#### tree_parser/expression.rs
1. `Operator2` に `LogicalAnd`, `LogicalOr` を追加
2. `Operator1` に `LogicalNot` を追加
3. 優先順位に従って新しいパース関数を追加:
   - `parse_to_expression_tree_logical_and` (比較より低い、論理ORより高い)
   - `parse_to_expression_tree_logical_or` (論理ANDより低い、代入より高い)
4. 単項演算子パースに `!` を追加

#### semantic_analyzer
- `Operator1`, `Operator2` の変換に新規オペレータを追加

#### interpreter
1. 短絡評価のため、`interpret_operation2` で特別な処理が必要
2. `LogicalAnd`: 左辺が0なら右辺を評価せず0を返す
3. `LogicalOr`: 左辺が非0なら右辺を評価せず1を返す
4. `LogicalNot`: 0なら1、非0なら0

**優先順位** (高い順):
1. 単項演算子 (`-`, `!`)
2. 乗除算 (`*`, `/`, `%`)
3. 加減算 (`+`, `-`)
4. 比較演算子 (`==`, `!=`, `<`, `<=`, `>`, `>=`)
5. 論理AND (`&&`)
6. 論理OR (`||`)
7. 代入 (`=`)

---

## 3. 剰余演算子

### 仕様 (spec.md 2.6)

```
7 % 3    # 1 (7 を 3 で割った余り) #
```

### 設計方針

**影響範囲**: token_parser, tree_parser, semantic_analyzer, interpreter

**変更内容**:

#### token_parser
1. `Token` 列挙型に `Percent` を追加
2. `parse_to_tokens_internal` で `%` をパース

#### tree_parser/expression.rs
1. `Operator2` に `Modulo` を追加
2. `parse_to_expression_tree_mul` に `%` の処理を追加 (`*`, `/` と同じ優先順位)

#### semantic_analyzer
- `Operator2::Modulo` の変換を追加

#### interpreter
- `Operator2::Modulo` の処理を追加 (`v1 % v2`)

---

## 4. 標準入出力

### 仕様 (spec.md 3.2)

```
__puti(42);     # 整数を出力 #
__putc(65);     # 文字（ASCII）を出力 #
let: n; n = __geti();  # 整数を入力 #
let: c; c = __getc();  # 1文字を入力 #
```

### 設計方針

**確認**: インタプリタのコードを確認したところ、`__puti`, `__putc`, `__geti`, `__getc` は既に実装済み。

**必要な作業**: 
- 既存のテストケースを確認し、動作を検証
- 必要に応じて追加のテストケースを作成

---

## 進捗

- [x] 設計ドキュメント作成
- [x] テストケース作成
- [x] 初回コミット
- [x] 文字リテラル実装
- [x] 論理演算子実装
- [x] 剰余演算子実装
- [x] 標準入出力確認（既に実装済み）
- [x] 全テスト実行・評価

## 実装完了日

2026年2月1日

## 実装結果

以下の機能を実装しました:

1. **文字リテラル** (spec 1.3)
   - `'A'` のような形式で文字のASCII値を表現
   - エスケープシーケンス: `\n`, `\r`, `\t`, `\s`, `\\`, `\'`
   - token_parser のみの変更

2. **論理演算子** (spec 2.4)
   - `!`: 論理NOT (0なら1、非0なら0)
   - `&&`: 論理AND (短絡評価)
   - `||`: 論理OR (短絡評価)
   - token_parser, tree_parser, interpreter を変更

3. **剰余演算子** (spec 2.6)
   - `%`: 剰余演算子 (乗除算と同じ優先順位)
   - token_parser, tree_parser, interpreter を変更

4. **標準入出力** (spec 3.2)
   - 既に実装済み (`__puti`, `__putc`, `__geti`, `__getc`)
   - テストで動作確認済み

### バグ修正

実装中に発見したバグを修正:
- `interpret_call_user_function` で関数の `return` 値が呼び出し元に伝播していた問題を修正
- 関数呼び出しの戻り値が式の値として正しく使用されるようになった
