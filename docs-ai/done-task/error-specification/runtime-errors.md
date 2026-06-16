# 実行時エラー (Runtime Errors)

## 概要

実行時エラーは、Whitespace プログラムの実行中に発生するエラーである。これらのエラーは、Whitespace インタプリタ（仮想マシン）によって検出される。

**実装場所**: `src/whitespace/interpreter.rs`  
**エラー型**: `RuntimeError` (enum)

## RuntimeError の定義

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// スタックアンダーフロー
    StackUnderflow,
    /// ゼロ除算
    DivisionByZero,
    /// 未定義ラベルへのジャンプ
    UndefinedLabel(i64),
    /// ヒープの未初期化アドレスへのアクセス
    UninitializedHeap(i64),
    /// コールスタックアンダーフロー（ret 命令でコールスタックが空）
    CallStackUnderflow,
    /// PC が命令列の範囲外
    ProgramCounterOutOfBounds,
    /// I/O エラー
    IoError(String),
    /// アサーション失敗（拡張 API）
    AssertionFailed(i64),
}
```

---

## エラー一覧

### 1. スタックアンダーフロー

**エラー種別**: `RuntimeError::StackUnderflow`

**発生条件**: データスタックが空の状態でスタック操作を試みる

**考えられる原因**:
- `pop` 命令の実行時にスタックが空
- 二項演算子の実行時に必要なオペランドが不足
- `swap`, `duplicate` などのスタック操作命令でスタックサイズが不足

**例**:
```whitespace
# スタックが空の状態で pop
[Tab][Lf][Tab][Tab]  # Output number (スタックから値を取得するが空)
```

nospace でのシナリオ（コンパイラバグがない限り通常は発生しない）:
```nospace
# コンパイラがバグっていて不適切なコード生成をした場合のみ
```

---

### 2. ゼロ除算

**エラー種別**: `RuntimeError::DivisionByZero`

**発生条件**: 除算または剰余演算で除数が 0

**Whitespace 命令**:
- `[Tab][Space][Tab][Space]` - Division
- `[Tab][Space][Tab][Tab]` - Modulo

**例**:
```nospace
func: main() {
  let: x;
  let: y;
  x = 10;
  y = 0;
  __clog(x / y);  # エラー: ゼロ除算
  return: 0;
}
```

---

### 3. 未定義ラベルへのジャンプ

**エラー種別**: `RuntimeError::UndefinedLabel(i64)`

**発生条件**: 存在しないラベルへの jump, jumpz, jumpn 命令を実行

**Whitespace 命令**:
- `[Lf][Space][Space]<label>` - Jump
- `[Lf][Tab][Space]<label>` - Jump if zero
- `[Lf][Tab][Tab]<label>` - Jump if negative

**考えられる原因**:
- コンパイラのバグ（ラベルの生成・参照ミスマッチ）
- 手書き Whitespace コードでのタイポ

**nospace では通常発生しない**（コンパイラが正しくラベルを管理している限り）

---

### 4. 未初期化ヒープアクセス

**エラー種別**: `RuntimeError::UninitializedHeap(i64)`

**発生条件**: ヒープメモリの未初期化アドレスから値を読み込もうとする

**Whitespace 命令**:
- `[Tab][Tab][Tab]` - Retrieve (ヒープから読み込み)

**例**:
```nospace
func: main() {
  let: ptr;
  let: value;
  ptr = 100;
  # ptr のアドレスに値を書き込んでいない
  value = __hload(ptr);  # エラー: 未初期化ヒープアクセス
  return: 0;
}
```

**注**: nospace の変数は自動的に 0 で初期化されるため、通常の変数では発生しない。ヒープ操作ビルトイン関数を使用した場合にのみ発生する。

---

### 5. コールスタックアンダーフロー

**エラー種別**: `RuntimeError::CallStackUnderflow`

**発生条件**: `ret` 命令実行時にコールスタックが空

**Whitespace 命令**:
- `[Lf][Tab][Lf]` - Return from subroutine

**考えられる原因**:
- コンパイラのバグ（call/ret のミスマッチ）
- 手書き Whitespace コードで `call` なしに `ret` を実行

**nospace では通常発生しない**（コンパイラが正しく関数呼び出しを管理している限り）

---

### 6. プログラムカウンタ範囲外

**エラー種別**: `RuntimeError::ProgramCounterOutOfBounds`

**発生条件**: プログラムカウンタが命令列の範囲外を指す

**考えられる原因**:
- コンパイラのバグ（不正なジャンプ先）
- VM の内部エラー（通常は発生しない）

**nospace では通常発生しない**

---

### 7. I/O エラー

**エラー種別**: `RuntimeError::IoError(String)`

**発生条件**: 入出力操作が失敗

**Whitespace 命令**:
- `[Tab][Lf][Space][Space]` - Output character
- `[Tab][Lf][Space][Tab]` - Output number
- `[Tab][Lf][Tab][Space]` - Read character
- `[Tab][Lf][Tab][Tab]` - Read number

**考えられる原因**:
- 標準入力の読み込み失敗
- 標準出力への書き込み失敗
- パイプやファイルリダイレクトのエラー

**例**:
```nospace
func: main() {
  let: x;
  x = __cin();  # エラー: 標準入力からの読み込みに失敗
  return: 0;
}
```

---

### 8. アサーション失敗

**エラー種別**: `RuntimeError::AssertionFailed(i64)`

**発生条件**: 拡張 API のアサーション命令が失敗

**説明**: これは nospace の拡張機能として実装されている可能性がある。詳細は実装を確認する必要がある。

**考えられる使用例**:
```nospace
func: main() {
  let: x;
  x = 10;
  __assert(x == 5);  # エラー: アサーション失敗（値は 10）
  return: 0;
}
```

**注**: この機能の実装状況は確認が必要。

---

## StepResult と実行フロー

VM の実行結果を表す enum:

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum StepResult {
    /// 実行継続中（バジェット消費で中断）
    Suspended,
    /// 正常終了（Exit 命令到達）
    Complete,
    /// 実行時エラー
    Error(RuntimeError),
}
```

### 実行モデル

1. `step(budget)` メソッドで指定ステップ数だけ実行
2. バジェット消費で `Suspended` を返して一時中断
3. エラー発生で `Error(RuntimeError)` を返す
4. Exit 命令で `Complete` を返す

---

## テストケースの網羅性

### 現在のテストカバレッジ（推測）

nospace のテストスイートで実行時エラーがテストされているかは不明。以下を確認する必要がある：

- [ ] ゼロ除算のテストケースが存在するか
- [ ] 未初期化ヒープアクセスのテストケースが存在するか
- [ ] I/O エラーのテストケースが存在するか

### 必要なテストケース

1. **ゼロ除算**
   ```nospace
   func: main() { return: 10 / 0; }
   ```

2. **未初期化ヒープアクセス**
   ```nospace
   func: main() { return: __hload(999); }
   ```

3. **I/O エラー**（stdin が空の状態で読み込み）
   ```nospace
   func: main() { return: __cin(); }
   ```

---

## 実装調査が必要な項目

1. **AssertionFailed の実装状況**
   - `__assert` ビルトイン関数が存在するか
   - どのような実装になっているか

2. **実行時エラーのテストカバレッジ**
   - 現在のテストスイートでどのエラーがカバーされているか
   - カバーされていないエラーは何か

3. **エラーメッセージのユーザー報告**
   - `RuntimeError` のメッセージがユーザーにどのように表示されるか
   - 詳細なデバッグ情報が提供されているか

---

## 改善提案

### 1. より詳細なエラーメッセージ

**現状**:
```
StackUnderflow
```

**改善案**:
```
Stack underflow at instruction 42 (pop operation)
  Stack size: 0
  Required: 1 value
```

### 2. エラー発生時のプログラム状態のダンプ

デバッグを容易にするため、エラー発生時に以下の情報を提供：

- プログラムカウンタ（PC）
- データスタックの内容
- コールスタックの内容
- 直前に実行した命令

### 3. ゼロ除算の自動検出（コンパイル時）

一部のゼロ除算は静的解析で検出可能：

```nospace
func: main() {
  return: 10 / 0;  # コンパイル時に警告またはエラー
}
```

---

## 関連ファイル

- `src/whitespace/interpreter.rs` - VM 実装と RuntimeError 定義
- `src/whitespace/parser.rs` - Whitespace パーサー（ParseError）
- `src/compiler_ws/builtin.rs` - 組み込みルーチン生成（ヒープ操作など）
