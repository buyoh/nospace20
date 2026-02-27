# if/while 式の戻り値機能 実装レポート

**日付**: 2026-02-07  
**ステータス**: ✅ 完了

## 概要

if と while が式として評価した値を返す機能を実装しました。これにより、より表現力の高いコードが書けるようになりました。

## 実装内容

### 1. インタプリタの変更 ([src/interpreter/mod.rs](../../src/interpreter/mod.rs))

#### 1.1. `interpret_statements_with_value()` メソッドの追加

```rust
fn interpret_statements_with_value(&mut self, statements: &Vec<ExecStatement>) -> (Flow, i64)
```

- ブロック内の文を順に実行し、最後の式の値とフロー制御情報を返す
- `last_value` を保持し、各式文の評価結果で更新
- `break`, `continue`, `return` が発生した場合、その時点の `last_value` を返す

#### 1.2. `interpret_if()` の変更

- 実行されたブロック(then または else)の `interpret_statements_with_value()` を呼び出し
- ブロックの最後の式の値を `ExpressionFlow::Value(value)` として返す
- フロー制御(return, break, continue)が発生した場合は `ExpressionFlow::Jump` を返す

#### 1.3. `interpret_while()` の変更

- `last_value` 変数を追加(初期値 0)
- 各イテレーションで `interpret_statements_with_value()` を呼び出し
- 通常終了の場合: `last_value` を更新
- break の場合: `last_value` を 0 に設定
- continue の場合: `last_value` を更新
- ループ終了後、`ExpressionFlow::Value(last_value)` を返す

### 2. ツリーパーサの変更 ([src/tree_parser/expression/mod.rs](../../src/tree_parser/expression/mod.rs))

#### 2.1. 優先度チェーンの変更

**変更前**:
```
root → assign → while → if → logical_or → ... → factor
```

**変更後**:
```
root → assign → logical_or → ... → factor (if/while を含む)
```

#### 2.2. `parse_to_expression_tree_factor()` の変更

- `Keyword::If` と `Keyword::While` のハンドリングを追加
- `parse_to_expression_tree_if_impl()` と `parse_to_expression_tree_while_impl()` を呼び出し
- if/while が関数引数や二項演算の中でも使用可能に

#### 2.3. 実装関数の追加

- `parse_to_expression_tree_if_impl()`: if 式のパース実装
- `parse_to_expression_tree_while_impl()`: while 式のパース実装

### 3. テストケースの追加

#### 3.1. if 式の戻り値テスト ([if_expr_value_001.ns](../../resources/tests/passes/control_flow/if_expr_value_001.ns))

- then ブロックの最後の式の値を返す
- else ブロックの最後の式の値を返す  
- 関数呼び出しの引数として if 式を使用
- ネストした if 式の動作
- 比較演算との組み合わせ

#### 3.2. while 式の戻り値テスト ([while_expr_value_001.ns](../../resources/tests/passes/control_flow/while_expr_value_001.ns))

- 最後のイテレーションの最後の式の値を返す
- ループが一度も実行されない場合は 0 を返す
- break で終了した場合は 0 を返す

## 仕様

### if 式の戻り値

- 実行されたブロック(then または else)の最後の式の値を返す
- else ブロックがなく、条件が false の場合は 0 を返す

### while 式の戻り値

| ケース | 戻り値 |
|--------|--------|
| 通常終了 | 最後のイテレーションの最後の式の値 |
| ループなし (条件が最初から false) | 0 |
| break で終了 | 0 |
| continue | 通常のイテレーションとして処理 |

## 使用例

```nospace
func: main() {
  let:x;
  let:i;
  
  # if 式の戻り値 #
  x = if: 1 { 42; } else: { 0; };  # x = 42 #
  
  # while 式の戻り値 #
  i = 0;
  x = while: i - 3 {
    i = i + 1;
    i;
  };  # x = 3 (最後のイテレーションの i の値) #
  
  # 関数引数としての if 式 #
  __assert(if: 1 { 5; } else: { 0; } == 5);
}
```

## テスト結果

- 全テストが成功: 72 passed
- 新規追加テスト:
  - `test_control_flow_if_expr_value_001`: ✅ PASS
  - `test_control_flow_while_expr_value_001`: ✅ PASS

## コミット情報

**コミットハッシュ**: 1ba4a35  
**コミットメッセージ**: 実装: if/while 式の戻り値機能

## 関連ドキュメント

- [ai-docs/task/unimplemented-syntax-features.md](../task/unimplemented-syntax-features.md) - セクション 2 を ✅ 実装済みに更新
- [docs/spec.md](../../docs/spec.md) - セクション 6.1, 6.2 (将来的に更新予定)

## 備考

### 設計上の決定

1. **break 時の戻り値を 0 にした理由**:
   - nospace には Rust のような `break 値;` 構文がない
   - シンプルで予測可能な動作
   - 0 は「値なし」を表現する慣例的な値

2. **ファクターレベルでの解析**:
   - if/while を優先度チェーンから外し、ファクターレベルで解析
   - 関数呼び出しや演算子の引数として自然に使用可能
   - 括弧なしで使用できる

### 今後の拡張可能性

- `break 値;` 構文の追加により、break 時の戻り値をカスタマイズ可能に
- 型システム導入時、if/else の両ブロックの型が一致することをチェック
