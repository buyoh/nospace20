# 内部型システム - 設計概要とフェーズ分割

## 実装フェーズ

### Phase 1: 型定義と型推論の基盤（semantic_analyzer）

**目標**: `ValueType` enum を追加し、各式の型を推論できるようにする

1. `ValueType` enum を `src/semantic_analyzer/types.rs` に追加
2. `ExecExpression` に型情報を持たせる仕組みを導入
3. `Function` に `return_type: ValueType` フィールドを追加
4. `FunctionIndex` に戻り値型を追加

### Phase 2: 型チェック（semantic_analyzer）

**目標**: void 式の不正使用をコンパイルエラーとして検出

1. 式変換時に型推論ロジックを追加
2. void 式が int を必要とする文脈で使用された場合にエラーを発行
3. ユーザー定義関数の戻り値型を推論する仕組みを追加
4. return 文の型チェックを追加

### Phase 3: interpreter の対応

**目標**: 型チェック済みコードの実行で問題がないことを確認

- semantic_analyzer が型チェックを完了した時点で、不正な型使用は排除されている
- interpreter は **実行時の値としては従来どおり `i64` を使用**（void 式は内部的に 0 を返すが、semantic_analyzer により値が使われるコードは拒否される）
- `ExpressionFlow` に変更は不要（型安全性は semantic_analyzer が保証）

### Phase 4: compiler_ws の対応

**目標**: void 型の式が効率的にコンパイルされるようにする

1. void 式がスタックに値を残さないようにする
2. void 式文では `Discard` を省略
3. void 関数の return でデフォルト値 `Push(0)` を省略
4. `generate_block` で void ブロックの処理を分岐

### Phase 5: テスト修正・追加

**目標**: 既存テストの修正と新規テストの追加

1. 影響を受ける既存テストの修正
2. void 型エラーを検出する新規テスト追加
3. void 型が正しく許可される文脈のテスト追加

## 設計方針

### 型情報の管理方法

式の型情報を `ExecExpression` に直接埋め込むのではなく、**型推論関数を使って式の型を計算する**方式を採用する。

理由:
- `ExecExpression` の全バリアントに型タグを追加すると、構造が複雑化する
- 式の型は構造的に決定できる（While → void, Factor → int, etc.）ため、型推論関数で十分
- compiler_ws が式のコード生成時に型を参照する必要がある場面は限定的

```rust
/// 式の型を推論する
pub(crate) fn infer_type(expr: &ExecExpression, functions: &[Function]) -> ValueType {
    match expr {
        ExecExpression::Factor(_) => ValueType::Int,
        ExecExpression::Variable(_) => ValueType::Int,
        ExecExpression::While(_, _) => ValueType::Void,
        ExecExpression::If(_, then_block, else_block) => {
            // 両ブロックが int の場合のみ int
            // ...
        }
        ExecExpression::UserFunction(id_ref, _) => {
            // 関数の戻り値型を参照
            // ...
        }
        // ...
    }
}
```

### ユーザー定義関数の戻り値型推論

2パスで推論する:

**パス1（既存パス1aに統合）**: 関数宣言の走査時に、本体を浅くスキャンして `return:` 文の有無を確認。この時点では int/void のどちらかを仮決定する。

**パス2（既存パス2に統合）**: 関数本体の変換時に、return 文の式の型が関数の戻り値型と整合するか検証。

```
function body に return: expr; が存在する → int
function body に return: がない → void
return: expr; と return: なし終端が混在 → error
```

注: `return: expr;` の有無は、ネストしたブロック・if・while の中も含めて再帰的にスキャンする。ただし **ネストした関数宣言の中は除外** する。

### エラーメッセージ

```
semantic error: cannot use void expression as a value
semantic error: cannot assign void expression to variable
semantic error: function 'foo' returns void; cannot use return value
semantic error: function 'bar' has mixed return types: some paths return a value, others do not
```

## 実装順序

1. Phase 1 → Phase 2 → Phase 5（semantic_analyzer + テスト修正）
2. Phase 3（interpreter 確認）
3. Phase 4（compiler_ws 対応）

Phase 1-2 と Phase 5 は密結合のため同時に進める。Phase 3 は確認のみで変更が少ない見込み。Phase 4 は独立して実施可能。
