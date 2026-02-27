# Whitespace コンパイラ: ビルトイン関数の実装

## 現状

whitespace へのコンパイル環境は構築されました:
- wsc (whitespacers) のインストール完了
- テストユーティリティ実装完了
- wsc統合テストの追加完了

## 問題

wsc統合テストを実行すると、ビルトイン関数が未定義というエラーが発生します:

```
called `Result::unwrap()` on an `Err` value: "Undefined function: __puti"
called `Result::unwrap()` on an `Err` value: "Undefined function: __putc"
called `Result::unwrap()` on an `Err` value: "Undefined function: __geti"
```

## 必要な実装

以下のビルトイン関数をコンパイラに実装する必要があります:

### 1. `__puti(value: int)` - 整数出力
- Whitespace命令: `OutputNumber` (TB LF SP TB)
- スタックから値を取り出して整数として出力

### 2. `__putc(value: int)` - 文字出力
- Whitespace命令: `OutputChar` (TB LF SP SP)
- スタックから値を取り出してASCII文字として出力

### 3. `__geti() -> int` - 整数入力
- Whitespace命令: `InputNumber` (TB LF TB TB)
- 標準入力から整数を読み取り、スタックに積む
- ヒープアドレスに格納する必要がある場合は適切に処理

### 4. `__getc() -> int` - 文字入力
- Whitespace命令: `InputChar` (TB LF TB SP)
- 標準入力から1文字読み取り、ASCII値をスタックに積む
- ヒープアドレスに格納する必要がある場合は適切に処理

## 実装箇所

### src/compiler_ws/builtin.rs

現在、このファイルには以下の実装があります:
- `generate_builtin_declarations()`

これを拡張して、ビルトイン関数の実装を生成する必要があります。

### src/compiler_ws/instruction.rs

以下の命令は既に定義されていますが、未使用の警告が出ています:
```rust
OutputChar,          // TB LF SP SP
OutputNumber,        // TB LF SP TB
InputChar,           // TB LF TB SP
InputNumber,         // TB LF TB TB
```

これらを実際に使用する必要があります。

## 実装方針

### オプション A: ビルトイン関数として実装

各ビルトイン関数を通常の関数として実装し、`builtin.rs` で宣言する:
- `__puti` → OutputNumber 命令を含む関数
- `__putc` → OutputChar 命令を含む関数
- `__geti` → InputNumber 命令を含む関数（ヒープ経由で値を返す）
- `__getc` → InputChar 命令を含む関数（ヒープ経由で値を返す）

### オプション B: コンパイラでインライン展開

ビルトイン関数呼び出しを検出したら、直接対応する命令を生成する:
- `src/compiler_ws/expression.rs` の関数呼び出し処理で特別扱い

### 推奨: オプション A

理由:
- 関数として実装することで、通常の関数呼び出し機構をテストできる
- 実装が明確で保守しやすい
- 後でインライン化の最適化も可能

## テストケース

以下のテストが動作することを確認:

1. `test_compile_and_run_puti` - `__puti(42)` で "42" を出力
2. `test_compile_and_run_putc` - `__putc(65)` で "A" を出力
3. `test_compile_and_run_arithmetic` - `__puti(1 + 2 * 3)` で "7" を出力
4. `test_compile_and_run_variable` - 変数経由で `__puti` を呼び出し
5. `test_compile_and_run_geti` - 入力 "10\n" → 出力 "20"

## 関連ファイル

- [src/compiler_ws/builtin.rs](../../src/compiler_ws/builtin.rs)
- [src/compiler_ws/instruction.rs](../../src/compiler_ws/instruction.rs)
- [src/compiler_ws/expression.rs](../../src/compiler_ws/expression.rs)
- [tests/compile_test.rs](../../tests/compile_test.rs)
- [tests/common/mod.rs](../../tests/common/mod.rs)

## 参考資料

- [docs/spec-whitespace.md](../../docs/spec-whitespace.md) - Whitespace言語仕様
- [ai-docs/task/compiler/test-strategy.md](test-strategy.md) - テスト戦略
