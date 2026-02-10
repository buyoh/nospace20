# 参照・デリファレンス実装完了レポート (Phase 1-3)

## 完了日

2026-02-10

## 概要

spec.md セクション 2.7 に定義される参照(`&`)・デリファレンス(`*`)演算子の実装が Phase 1-3 において完了しました。インタプリタでの実行が可能になり、全てのテストが通過しています。

## 実装内容

### Phase 1: 基盤整備

**完了日**: 2026-02-08

#### token_parser の変更
- `Token` enum に `Ampersand` バリアントを追加
- `&` のパース処理を変更し、単独の `&` を `Token::Ampersand` として扱うように修正
  - 従来: 単独の `&` はエラー
  - 変更後: `&` → `Token::Ampersand`、`&&` → `Token::DoubleAmpersand`

#### tree_parser の変更
- `Operator1` enum に `Ref`（参照）と `Deref`（デリファレンス）を追加
- `parse_to_expression_tree_unary` 関数を拡張し、`&` と `*` を単項演算子として処理
  - `Token::Ampersand` → `Operator1::Ref`
  - `Token::Asterisk` → `Operator1::Deref`（既存の乗算と区別される）

#### grammar.bnf の更新
- 単項演算子の定義を拡張: `("-" | "!" | "&" | "*") expr_unary`
- 未実装機能リストから「参照 (&x)」と「間接参照 (*p)」を削除

#### テストの追加
- `src/token_parser/test.rs` - `&` の単独トークン化テスト
- `src/tree_parser/expression/test.rs` - `&x`, `*p`, `**p`, `a * *p` のパーステスト

### Phase 2: 意味解析

**実装**: `src/semantic_analyzer/mod.rs`

- `Operator1::Ref` の変換処理を実装
- `&` の対象が変数または配列アクセスであることの検証
- グローバル変数・ローカル変数・配列要素への参照取得をサポート

### Phase 3: インタプリタ

**実装**: `src/interpreter/exec.rs`

#### アドレス空間の設計
- 全変数（グローバル・ローカル）を統一的なフラットアドレス空間にマッピング
- アドレス空間: `[0..global_count) [global_count..global_count+scope0_size) ...`

#### 実装した機能
1. **`resolve_address(id: &IdentifierRef) -> i64`**
   - 変数識別子からアドレスを計算
   - グローバル変数とローカル変数を区別して処理

2. **`get_by_address(addr: i64) -> i64`**
   - アドレスから値を取得
   - グローバル変数領域とローカル変数領域を適切に処理

3. **`set_by_address(addr: i64, value: i64)`**
   - アドレスに値を設定
   - デリファレンス代入 (`*ptr = value`) をサポート

4. **`Operator1::Ref` の評価**
   - 変数のアドレスを計算して返す
   - 配列要素への参照もサポート (`&arr[i]`)

5. **`Operator1::Deref` の評価**
   - アドレスから値を読み取る
   - 左辺値としての使用もサポート (`*ptr = value`)

#### 関数呼び出しの修正
関数間でアドレスを渡すために `interpret_call_user_function` を修正:

**修正前**: 新しい `LocalEnvironment` を作成（呼び出し元の変数が見えない）
**修正後**: 既存の `scope_stack` に新しい関数のスコープを push（呼び出し元の変数も見える）

```rust
// 新しい scope を既存の scope_stack に push
let mut variables = vec![0; func.block.scope.variable_count];
// ... 変数初期化 ...
self.scope_stack.push(variables);

// 関数本体を実行
let result = match self.interpret_statements(&func.block.statements) { ... };

// 関数スコープを pop
self.scope_stack.pop();
```

この変更により、関数間でアドレスを渡すテストが成功するようになった。

## テスト結果

### 統合テスト

参照・デリファレンスに関する5つのテストが全て PASS:

1. `test_operators_ref_basic_001` - 基本的な参照・デリファレンス
2. `test_operators_ref_deref_assign_001` - デリファレンス代入 (`*ptr = value`)
3. `test_operators_ref_double_001` - ダブルデリファレンス (`**pp`)
4. `test_operators_ref_func_arg_001` - 関数引数として参照を渡す
5. `test_operators_ref_swap_001` - 参照を使った swap 関数

### 全体テスト結果

```
cargo test --quiet
running 116 tests
test result: ok. 102 passed; 0 failed; 14 ignored
```

全てのテストが成功し、既存機能への影響も確認されませんでした。

## 仕様との対応

### spec.md セクション 2.7

```
&x       # 変数 x の参照（アドレス）を取得 #
*p       # 参照 p をデリファレンス（間接参照） #
```

- `&` : 変数の参照を取得する。変数に対してのみ使用可能。
- `*` : 参照をデリファレンスして、参照先の値を取得または代入する。
- 参照はC言語のポインタに似ているが、本言語では「参照」と呼ぶ。
- スタック上の変数、static変数、グローバル変数など制限なく参照を取得できる。
- 参照先が開放済みの場合の動作は未定義。特にプロテクションは行わない。

これらの仕様が全て実装され、動作確認されました。

## 未完了の項目

### Phase 4: Whitespace コンパイラ

`src/compiler_ws/expression.rs` において、`Operator1::Ref` と `Operator1::Deref` は `unimplemented!()` のままです。

Whitespace コンパイラでの実装方針:
- Whitespace はヒープベースのアーキテクチャ
- 変数は全てヒープアドレスで管理されている
- `&var` → 変数のヒープアドレス整数値をスタックに Push
- `*ptr` → スタックトップの値をアドレスとして `Retrieve` 命令
- `*ptr = val` → アドレスと値をスタックに積んで `Store` 命令

この実装は将来の Phase 4 で行われる予定です。

## 技術的詳細

### アドレスエンコーディング

```
グローバル変数: address = index (0から順番)
ローカル変数: address = global_count + Σ(前のスコープのサイズ) + index
```

### スコープスタックの管理

関数呼び出し時に新しいスコープを `scope_stack` に push し、復帰時に pop することで、関数間でのアドレス共有を実現しています。これにより、呼び出し元のローカル変数への参照を呼び出し先で使用できます。

### ダングリングポインタ

関数のローカル変数への参照を return で返した場合、呼び出し元では pop 済みのスコープを指すため、ダングリングポインタとなります。C 言語同様に未定義動作として扱い、特にプロテクションは行いません。

## 関連ドキュメント

- `ai-docs/task/reference-dereference/overview.md` - 全体設計
- `ai-docs/task/reference-dereference/phase1-implementation-report.md` - Phase 1 詳細レポート
- `ai-docs/task/reference-dereference/phase3-failure-analysis.md` - Phase 3 失敗分析と修正方針
- `ai-docs/task/reference-dereference/interpreter.md` - インタプリタ実装設計
- `ai-docs/task/reference-dereference/compiler-ws.md` - Whitespace コンパイラ実装設計（未実装）

## 備考

参照・デリファレンスの実装により、将来の配列実装（spec.md セクション 4.2）の基盤が整いました。配列は内部的に `*(base + i)` で実装可能であり、参照が前提技術となります。
