# compiler_ws モジュール変更設計

## 対象ファイル

- `src/compiler_ws/expression.rs`（主要変更）
- `src/compiler_ws/memory.rs`（変更なし、既存のアドレス体系を活用）

## 現状

### メモリレイアウト

Whitespace はヒープベースのアーキテクチャ。変数はヒープアドレスで管理される。

```
[0..7]       → 予約（LOCAL_HEAP_BEGIN=2, LOCAL_HEAP_END=3, TEMP_PTR=4）
[8..]        → GLOBAL_PTR: グローバル変数領域
[heap_end..] → ローカル変数（動的に拡張）
```

### 変数アクセスのコード生成

- `generate_load_variable(id)` → ヒープアドレスを計算し `Retrieve` 命令でスタックに値を積む
- 代入 → ヒープアドレスと値をスタックに積んで `Store` 命令

### 式のコード生成

`generate_expression` が `ExecExpression` を match し、各バリアントに対応するコードを生成。

### 単項演算子のコード生成

`generate_unary_op` が `Operator1::Negative` / `LogicalNot` を処理。

## 変更内容

### 1. `&var` のコード生成（Operator1::Ref）

参照 `&var` は「変数のヒープアドレス値をスタックに積む」操作。

```rust
Operator1::Ref => {
    // 内部式は Variable(IdentifierRef) であること保証済み（意味解析で検証）
    if let ExecExpression::Variable(id_ref) = inner.as_ref() {
        // 変数のヒープアドレスをスタックに Push
        self.generate_push_variable_address(id_ref);
    } else {
        panic!("compiler error: & applied to non-variable");
    }
}
```

`generate_push_variable_address` は既存の `generate_load_variable` と類似だが、`Retrieve` 命令を発行しない（アドレスだけを積む）。

既存の `generate_load_variable` の処理:
1. 変数のヒープアドレスを計算してスタックに Push
2. `Retrieve` 命令で値を取得

新しい `generate_push_variable_address`:
1. 変数のヒープアドレスを計算してスタックに Push（`Retrieve` なし）

### 2. `*ptr` のコード生成（Operator1::Deref）

デリファレンス `*ptr` は「スタックトップの値をアドレスとして `Retrieve`」操作。

```rust
Operator1::Deref => {
    // 内部式を評価（アドレス値がスタックトップに残る）
    self.generate_expression(inner);
    // スタックトップの値をアドレスとしてヒープから読み取り
    self.emit(Instruction::Retrieve);
}
```

### 3. `*ptr = value` のコード生成

代入の左辺がデリファレンスの場合:

```rust
Operator2::Assign => {
    match left.as_ref() {
        ExecExpression::Variable(id_ref) => {
            // 既存の代入処理
        }
        ExecExpression::Operation1(Operator1::Deref, inner) => {
            // アドレスを計算してスタックに積む
            self.generate_expression(inner);
            // 値を計算してスタックに積む
            self.generate_expression(right);
            // Store 命令（スタック: [addr, value] → ヒープ[addr] = value）
            self.emit(Instruction::Store);
            // 代入式の結果として値をもう一度スタックに積む
            // （Store はスタックから消費するため、値が必要なら再計算）
            self.generate_expression(inner);
            self.emit(Instruction::Retrieve);
            // TODO: 上記は非効率。TEMP_PTR を使用して最適化可能
        }
        _ => panic!("compiler error: invalid lvalue"),
    }
}
```

注意: Whitespace の `Store` 命令はスタックから `[address, value]` を消費する。代入式が値を返す場合は、Store 後に再度値を取得する必要がある。`TEMP_PTR` を使った最適化が可能。

### 4. generate_unary_op の拡張

```rust
fn generate_unary_op(&mut self, op: &Operator1, inner: &ExecExpression) {
    match op {
        Operator1::Negative => { /* 既存 */ }
        Operator1::LogicalNot => { /* 既存 */ }
        Operator1::Ref => {
            // &var: 変数アドレスを Push（Retrieve なし）
            if let ExecExpression::Variable(id_ref) = inner {
                self.generate_push_variable_address(id_ref);
            }
        }
        Operator1::Deref => {
            // *ptr: 式を評価して Retrieve
            self.generate_expression(inner);
            self.emit(Instruction::Retrieve);
        }
    }
}
```

## Whitespace における参照の自然さ

Whitespace はスタックマシン + ヒープモデル。全変数がヒープ上のアドレスで管理されるため:

- `&var` → ヒープアドレスの即値をスタックに Push
- `*ptr` → スタックトップの値をアドレスとして `Retrieve`
- `*ptr = val` → アドレスと値をスタックに積んで `Store`

これはWhitespace の命令セットそのものであり、参照の実装は非常に自然。

## テスト

compiler_ws のテストは、compile_test.rs の Whitespace 統合テストで行う。参照用の `.ns` テストケースを追加し、Whitespace コンパイル→実行が正しく動作することを検証する。
