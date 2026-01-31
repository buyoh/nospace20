# テストケースのカテゴリ分けと拡充

## 概要

resources/tests のテストケースをカテゴリ分けし、テストを拡充するタスク。

## 現状分析

### 既存のテストケース

- c000.ns: 基本的な`__trace`のテスト
- c001.ns: 変数、四則演算、単項演算子のテスト
- c002.ns: while文、if文、break、continueのテスト
- c003.ns: 比較演算子のテスト
- c004.ns: スコープ(ブロックスコープ内の変数定義)のテスト

### 言語仕様に基づくカテゴリ

spec.md に基づき、以下のカテゴリを定義:

1. **literals** (リテラル・識別子)
   - 数値リテラル
   - 識別子
   - コメント

2. **operators** (演算)
   - 四則演算
   - 単項演算子
   - 比較演算子
   - 演算子の優先順位

3. **builtins** (組み込み識別子)
   - `__clog`
   - `__assert`
   - `__assert_not`
   - `__trace`

4. **variables** (代入・変数定義)
   - 変数定義
   - 代入
   - ホイスティング
   - (未実装) final/const
   - (未実装) 初期値指定
   - (未実装) グローバル変数

5. **functions** (関数定義)
   - 関数定義
   - 関数呼び出し
   - 引数
   - 戻り値
   - ホイスティング

6. **control_flow** (制御構文)
   - while文
   - if/else文
   - break/continue
   - return

7. **scope** (スコープ)
   - ブロックスコープ
   - 関数スコープ
   - ネストした関数定義
   - (未実装) ブロックスコープ内での変数定義

8. **integration** (統合テスト)
   - 複数機能の組み合わせ

## テストディレクトリ構造案

```
resources/tests/
  literals/
    num_001.ns         - 基本的な数値リテラル
    num_002.ns         - 負の数
    ident_001.ns       - 識別子の命名規則
    comment_001.ns     - コメント
  operators/
    arith_001.ns       - 基本的な四則演算
    arith_002.ns       - 優先順位
    arith_003.ns       - 括弧
    unary_001.ns       - 単項マイナス
    compare_001.ns     - 比較演算子(既存のc003相当)
  builtins/
    trace_001.ns       - __trace
    assert_001.ns      - __assert, __assert_not
    clog_001.ns        - __clog
  variables/
    var_basic_001.ns   - 変数定義と代入(既存のc001相当)
    var_hoist_001.ns   - ホイスティング
    [未実装]var_global_001.ns - グローバル変数
  functions/
    func_basic_001.ns  - 基本的な関数定義
    func_args_001.ns   - 引数
    func_return_001.ns - 戻り値
    func_hoist_001.ns  - ホイスティング
    func_nested_001.ns - ネストした関数定義
  control_flow/
    while_001.ns       - while文
    if_001.ns          - if文
    break_continue_001.ns - break/continue(既存のc002相当)
    return_001.ns      - return文
  scope/
    scope_block_001.ns - ブロックスコープ(既存のc004相当)
    scope_func_001.ns  - 関数スコープ
    [未実装]scope_nested_001.ns - ネストしたスコープでの変数定義
  integration/
    integ_001.ns       - 複数機能の組み合わせ
```

## 未実装テストの扱い

- テストファイル名の先頭に `[未実装]` または `disabled_` プレフィックスを付ける
- テストコードで除外可能にする仕組みを実装

## 進捗

- [x] 言語仕様の分析
- [x] カテゴリの定義
- [x] テストディレクトリ構造の設計
- [x] 新しいテストケースの作成
- [x] テストコードの更新
- [x] 未実装テストの除外機能の実装

## 結果

### 作成したテストケース

**Literals (リテラル):**
- num_001, num_002: 数値リテラル
- ident_001: 識別子の命名規則
- comment_001: コメント

**Operators (演算子):**
- arith_001, arith_002, arith_003: 四則演算と優先順位
- unary_001: 単項マイナス演算子
- compare_001: 比較演算子

**Builtins (組み込み識別子):**
- trace_001: `__trace`
- assert_001: `__assert`, `__assert_not`

**Variables (変数):**
- var_basic_001: 変数定義と代入
- var_hoist_001: ホイスティング
- disabled_var_global_001: グローバル変数(未実装)
- disabled_var_final_001: final変数(未実装)
- disabled_var_init_001: 初期値指定(未実装)

**Functions (関数):**
- func_basic_001: 基本的な関数定義
- func_args_001: 引数 (**要修正**)
- func_return_001: 戻り値 (**要修正**)
- func_hoist_001: ホイスティング
- func_nested_001: ネストした関数定義 (**要修正**)

**Control Flow (制御構文):**
- while_001: while文
- if_001: if/else文 (**要修正**)
- break_continue_001: break/continue (**除外: ハング**)
- return_001: return文 (**要修正**)

**Scope (スコープ):**
- scope_block_001: ブロックスコープ (**未実装機能**)
- scope_func_001: 関数スコープ (**要修正**)
- scope_nested_func_001: ネストした関数スコープ (**要修正**)
- disabled_scope_block_var_001: ブロックスコープ内変数定義(未実装)

**Integration (統合テスト):**
- integ_001: 複数機能の組み合わせ (**未実装機能含む**)

### テスト結果

- **成功: 19テスト**
- **失敗: 10テスト** (未実装機能や要修正)
- **除外: 2テスト** (c002, break_continue_001 - ハング)
- **未実装テスト: 4テスト** (コメントアウト済み)

### 既知の問題

1. **break/continue**: c002とbreak_continue_001がハングするため除外
2. **ブロックスコープ内変数定義**: 未実装 (c004, scope_block_001, integration_integ_001が失敗)
3. **関数テスト**: 一部トレースの不一致あり (要調査)
4. **else構文**: スペースの扱いに注意が必要

