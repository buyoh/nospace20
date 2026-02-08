# 参照・デリファレンス実装 - 全体概要

## 背景

spec.md セクション 2.7 に以下の仕様が「未実装」として定義されている:

```
&x       # 変数 x の参照（アドレス）を取得
*p       # 参照 p をデリファレンス（間接参照）
```

- `&` : 変数の参照を取得する。変数に対してのみ使用可能。
- `*` : 参照をデリファレンスして、参照先の値を取得または代入する。
- 参照はC言語のポインタに似ているが、本言語では「参照」と呼ぶ。

さらに、配列（セクション 4.2、未実装）では `&arr` が配列の先頭要素の参照を取得する用途で使われる。

## 設計方針

### 値の表現: i64 にアドレスを埋め込む

現在のシステムでは、全ての値が `i64` で表現されている（`ExpressionFlow::Value(i64)`, `Vec<i64>` による変数ストレージ）。参照の実装にあたり、以下の2方式を比較した結果、**方式A（i64埋め込み）** を採用する。

| 観点 | 方式A: i64埋め込み | 方式B: Value enum化 |
|------|-------------------|-------------------|
| 変更範囲 | 小（演算子追加のみ） | 大（全モジュール波及） |
| 型安全性 | なし（C言語相当） | あり |
| spec準拠 | 「C言語のポインタに似ている」と合致 | spec以上の安全性 |
| compiler_ws互換 | 自然（WSヒープアドレスもi64） | WS側で無意味な抽象化 |
| 配列との整合 | 配列ポインタ演算と整合 | 複雑化 |

#### アドレスのエンコーディング（インタプリタ）

インタプリタでは変数が `scope_stack: Vec<Vec<i64>>` と `global_variables: Vec<i64>` に格納されている。
参照値を i64 にエンコードするスキームが必要。

```
エンコード方式:
  ローカル変数: address = scope_absolute_index * MAX_VARS + local_index
  グローバル変数: address = GLOBAL_MARKER + local_index

デコード方式:
  address >= GLOBAL_MARKER → グローバル変数
  それ以外 → (scope_absolute_index, local_index) を算出
```

ただし `scope_depth`（相対値）と `scope_stack.len()`（絶対値）の変換が必要。

**簡易方式（推奨）**: 全変数を統一的なフラットアドレス空間にマッピングする。

```
アドレス空間:
  [0, global_count)                          → グローバル変数
  [global_count, global_count + local_0)     → スコープ0のローカル変数
  [global_count + local_0, ...)              → スコープ1のローカル変数
  ...

&var → 変数の絶対アドレスを計算して返す
*ptr → アドレスからスコープとインデックスを逆算して値を取得
*ptr = val → 同上で値を設定
```

インタプリタに `resolve_address(id: &IdentifierRef) -> i64` と `access_by_address(addr: i64) -> &mut i64` を追加する。

### compiler_ws での実装

Whitespace はヒープベースのアーキテクチャ。変数は全てヒープアドレスで管理されているため、参照の実装は自然に行える:

- `&var` → 変数のヒープアドレス整数値をスタックに Push
- `*ptr` → スタックトップの値をアドレスとして `Retrieve` 命令
- `*ptr = val` → アドレスと値をスタックに積んで `Store` 命令

### 代入の左辺としてのデリファレンス

`*ptr = value;` をサポートするため、代入の左辺に `*expr` を許容する必要がある。
現在は `Operator2::Assign` の左辺が `ExecExpression::Variable` のみ。

```rust
// 現在
if let ExecExpression::Variable(id_ref) = expr1.as_ref() { ... }

// 拡張後
match expr1.as_ref() {
    ExecExpression::Variable(id_ref) => { /* 通常代入 */ }
    ExecExpression::Operation1(Operator1::Deref, inner) => { /* デリファレンス代入 */ }
    _ => panic!("left value is not assignable")
}
```

## 実装フェーズ

### Phase 1: 基盤整備
1. token_parser: `Ampersand` トークン追加
2. tree_parser: `Operator1::Ref` / `Operator1::Deref` 追加、パーサ拡張
3. grammar.bnf 更新

### Phase 2: 意味解析
4. semantic_analyzer: `Operator1::Ref` / `Deref` の変換処理
5. `&` の対象が変数であることの検証（意味解析時）

### Phase 3: インタプリタ
6. アドレス空間のエンコード/デコード実装
7. `Operator1::Ref` の評価（アドレス計算）
8. `Operator1::Deref` の評価（アドレスから値読み取り）
9. `*ptr = value` の代入対応

### Phase 4: Whitespace コンパイラ
10. `&var` のコード生成（ヒープアドレス Push）
11. `*ptr` のコード生成（Retrieve）
12. `*ptr = value` のコード生成（Store）

### Phase 5: テスト・ドキュメント
13. ユニットテスト追加
14. 統合テスト追加
15. spec.md の「(未実装)」表記削除
16. grammar.bnf のコメントアウト解除

## 配列との関係

配列（spec セクション 4.2）は未実装。参照は配列に先行して実装すべき理由:

1. `&scalar_var` だけでも有効な用途がある（関数へのポインタ渡し等）
2. 配列 `arr[i]` は内部的に `*(base + i)` で実装可能 → 参照が前提技術
3. 各パイプラインの拡張パターンが参照で確立され、配列実装が容易になる
4. 変更が小さく、段階的に検証可能

## 影響モジュール一覧

| モジュール | 影響度 | 詳細ドキュメント |
|-----------|--------|----------------|
| token_parser | 小 | [token-parser.md](token-parser.md) |
| tree_parser | 小 | [tree-parser.md](tree-parser.md) |
| semantic_analyzer | 中 | [semantic-analyzer.md](semantic-analyzer.md) |
| interpreter | 大 | [interpreter.md](interpreter.md) |
| compiler_ws | 中 | [compiler-ws.md](compiler-ws.md) |
| docs/grammar.bnf | 小 | [grammar-spec.md](grammar-spec.md) |

## リスク・注意点

1. **`*` のコンテキスト依存パース**: `Token::Asterisk` が乗算と単項デリファレンスで共用される。優先順位チェーン上、`unary` で先に消費するため問題ないが、エッジケースの検証が必要。
2. **アドレス空間の安定性**: スコープの入退場でアドレス空間が変動する。ダングリングポインタの検出は行わない（C言語相当の動作）。
3. **再帰関数でのアドレス**: 再帰呼び出しで同じ関数のローカル変数が複数インスタンス存在する場合のアドレス計算。
