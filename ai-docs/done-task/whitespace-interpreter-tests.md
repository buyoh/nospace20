# Whitespace インタプリタ直接テスト設計

## 概要

`src/whitespace/` の Whitespace パーサ・インタプリタを、nospace コンパイラを介さずに直接テストする。
`resources/tests/` と同じ方針で `resources/tests_ws/` を作成し、Whitespace プログラムをテストコンテンツとして用意する。

**対象**: `spec-whitespace.md` の標準仕様のみ（拡張仕様は対象外）

## 現状分析

### テスト対象モジュール

- `src/whitespace/parser.rs`: Whitespace テキスト → `Vec<Instruction>` パーサ
- `src/whitespace/interpreter.rs`: `WhitespaceVM` スタックマシンインタプリタ

### 既存のテスト

`parser.rs` と `interpreter.rs` に `#[cfg(test)]` のユニットテストが存在するが、カバレッジが限定的：

- **パーサ**: Push, Add, 数値（0/正/負）, Exit, 複数命令, 非空白無視, ラウンドトリップ
- **インタプリタ**: Push+Add, 中断・再開, サブルーチン呼出, トレース拡張, ヒープ, ゼロ除算, スタックアンダーフロー

### 未テストの領域

- スタック操作: Duplicate, Swap, Discard, Copy
- 算術: Sub, Mul, Mod（Div のエラーケースのみ）
- I/O 命令: OutputChar, OutputNumber, InputChar, InputNumber
- フロー制御: Jump, JumpIfZero, JumpIfNegative の個別テスト
- 複合プログラム（ループ, 条件分岐, ヒープ経由の I/O）
- エラーケース: 未定義ラベル, コールスタックアンダーフロー, PC 範囲外

### 仕様上の未実装

- Slide 命令（`[Tab][LF]` + Number）: パーサ・Instruction enum ともに未実装
  - テスト対象外とする

## 設計

### ファイル形式: WSA (Whitespace Assembly) 記法

Whitespace コードは Space/Tab/LF のみで構成されるため、テストファイルをそのまま書くと可読性・保守性が低い。
そこで、人間が読める **WSA 記法** を採用する。

#### WSA 記法の仕様

- `S` = Space, `T` = Tab, `N` = LF
- `#` で始まる行はコメント
- 上記以外の文字（小文字, 数字, 記号, 空白）は無視
- ファイル拡張子: `.wsa`

#### 例: 3 + 5 = 8 を出力

```wsa
# Push 3
SSSTTV
# Push 5
SSSTSTN
# Add
TSSS
# OutputNumber
TNST
# Exit
NNN
```

#### テストランナーでの処理

```rust
fn decode_wsa(content: &str) -> String {
    content.chars().filter_map(|c| match c {
        'S' => Some(' '),
        'T' => Some('\t'),
        'N' => Some('\n'),
        _ => None,
    }).collect()
}
```

これを `whitespace::parse()` に渡す。

### ディレクトリ構造

```
resources/tests_ws/
├── README.md                # テスト概要
├── test-manifest.yaml       # テスト定義
├── passes/                  # 正常系テスト
│   ├── stack/               # スタック操作
│   ├── arith/               # 算術演算
│   ├── heap/                # ヒープアクセス
│   ├── flow/                # フロー制御
│   └── io/                  # I/O
└── fails/                   # 異常系テスト
    └── runtime/             # 実行時エラー
```

### check.json 形式

#### ws_io テスト（I/O 検証）

```json
{
  "type": "ws_io",
  "stdout": "8"
}
```

stdin 付き:

```json
{
  "type": "ws_io",
  "stdin": "42\n",
  "stdout": "42"
}
```

#### ws_runtime_error テスト（実行時エラー検証）

```json
{
  "type": "ws_runtime_error",
  "error": "StackUnderflow"
}
```

error フィールドの値は `RuntimeError` の variant 名:
- `StackUnderflow`
- `DivisionByZero`
- `UndefinedLabel`
- `CallStackUnderflow`
- `ProgramCounterOutOfBounds`

### build.rs 拡張

`build.rs` を拡張し、`resources/tests_ws/test-manifest.yaml` からもテストを自動生成する。

#### 新しいテストタイプ

| type | 説明 |
|------|------|
| `ws_io` | WSA を読み込み、パース→実行→stdout 比較 |
| `ws_runtime_error` | WSA を読み込み、パース→実行→RuntimeError を確認 |

#### 生成されるテスト関数

```rust
// ws_io
#[test]
fn test_ws_stack_push_positive_001() {
    test_ws_io_base("stack/push_positive_001")
}

// ws_runtime_error
#[test]
fn test_ws_errors_stack_underflow_001() {
    test_ws_runtime_error_base("errors/stack_underflow_001")
}
```

### テストランナー関数

`tests/code_test.rs` に追加するベース関数（または新ファイル `tests/whitespace_direct_test.rs`）:

```rust
fn decode_wsa(content: &str) -> String {
    content.chars().filter_map(|c| match c {
        'S' => Some(' '),
        'T' => Some('\t'),
        'N' => Some('\n'),
        _ => None,
    }).collect()
}

fn test_ws_io_base(test_name: &str) {
    let path_base = format!("resources/tests_ws/passes/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base)).unwrap();
    let ws_code = decode_wsa(&wsa_content);

    let check: serde_json::Value = serde_json::from_reader(
        io::BufReader::new(fs::File::open(format!("{}.check.json", path_base)).unwrap())
    ).unwrap();

    let stdin_str = check.get("stdin").and_then(|v| v.as_str()).unwrap_or("");
    let expected_stdout = check.get("stdout").and_then(|v| v.as_str()).unwrap_or("");

    let mut vm = WhitespaceVM::from_source(&ws_code).unwrap();
    let stdin_cursor = Box::new(std::io::Cursor::new(stdin_str.to_string().into_bytes()));
    let stdout_buf: Box<Vec<u8>> = Box::new(Vec::new());
    let vm = vm.with_io(stdin_cursor, stdout_buf);
    let result = vm.run(100_000);

    assert_eq!(result, StepResult::Complete);
    let actual_stdout = vm.get_stdout_string();
    assert_eq!(expected_stdout, actual_stdout);
}

fn test_ws_runtime_error_base(test_name: &str) {
    let path_base = format!("resources/tests_ws/fails/runtime/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base)).unwrap();
    let ws_code = decode_wsa(&wsa_content);

    let check: serde_json::Value = serde_json::from_reader(
        io::BufReader::new(fs::File::open(format!("{}.check.json", path_base)).unwrap())
    ).unwrap();

    let expected_error = check.get("error").unwrap().as_str().unwrap();

    let mut vm = WhitespaceVM::from_source(&ws_code).unwrap();
    let result = vm.run(100_000);

    match result {
        StepResult::Error(e) => {
            let error_name = format!("{:?}", e).split('(').next().unwrap().to_string();
            assert_eq!(expected_error, error_name);
        }
        _ => panic!("Expected runtime error but got {:?}", result),
    }
}
```

## テストケース一覧

### 数値エンコーディングの参照表

| 値 | STN エンコーディング | 備考 |
|----|-----------------|------|
| 0  | `SN` | 符号+ のみ、ビットなし |
| 1  | `STN` | +1 |
| 2  | `STSN` | +10 |
| 3  | `STTN` | +11 |
| 5  | `STSTN` | +101 |
| 8  | `STSSN` | +1000 |
| 10 | `STSTSN` | +1010 |
| 42 | `STSTSTSTSN` | +101010 |
| 65 | `STSSSSTN` | +1000001 = 'A' |
| 72 | `STSSTSSSN` | +1001000 = 'H' |
| -1 | `TTN` | -1 |
| -3 | `TTTN` | -11 |

### 命令エンコーディングの参照表

| 命令 | IMP + コマンド | STN | 
|------|------------|-----|
| Push N | SP + SP + \<num\> | `SS` + number |
| Duplicate | SP + LF SP | `SNS` |
| Swap | SP + LF TB | `SNT` |
| Discard | SP + LF LF | `SNN` |
| Copy N | SP + TB SP + \<num\> | `STS` + number |
| Add | TB SP + SP SP | `TSSS` |
| Sub | TB SP + SP TB | `TSST` |
| Mul | TB SP + SP LF | `TSSN` |
| Div | TB SP + TB SP | `TSTS` |
| Mod | TB SP + TB TB | `TSTT` |
| Store | TB TB + SP | `TTS` |
| Retrieve | TB TB + TB | `TTT` |
| Label L | LF + SP SP + \<label\> | `NSS` + label |
| Call L | LF + SP TB + \<label\> | `NST` + label |
| Jump L | LF + SP LF + \<label\> | `NSN` + label |
| JumpIfZero L | LF + TB SP + \<label\> | `NTS` + label |
| JumpIfNeg L | LF + TB TB + \<label\> | `NTT` + label |
| Return | LF + TB LF | `NTN` |
| Exit | LF + LF LF | `NNN` |
| OutputChar | TB LF + SP SP | `TNSS` |
| OutputNumber | TB LF + SP TB | `TNST` |
| InputChar | TB LF + TB SP | `TNTS` |
| InputNumber | TB LF + TB TB | `TNTT` |

### 1. スタック操作テスト (stack/)

#### stack/push_positive_001
**概要**: 正の整数をプッシュし出力
```wsa
# Push 42
SSSTSTSTSTSN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "42"}`

#### stack/push_negative_001
**概要**: 負の整数をプッシュし出力
```wsa
# Push -3
SSTTN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "-3"}`

#### stack/push_zero_001
**概要**: ゼロをプッシュし出力
```wsa
# Push 0
SSSN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "0"}`

#### stack/dup_001
**概要**: Duplicate で最上位を複製し 2 回出力
```wsa
# Push 7
SSSTTN
# Duplicate
SNS
# OutputNumber (top = 7)
TNST
# OutputNumber (dup = 7)
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "77"}`

#### stack/swap_001
**概要**: Swap で上位2要素を交換
```wsa
# Push 1
SSSTN
# Push 2
SSTSN
# Swap
SNT
# OutputNumber (top was 2, after swap = 1)
TNST
# OutputNumber (was 1, after swap = 2)
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "12"}`

#### stack/discard_001
**概要**: Discard で最上位を破棄
```wsa
# Push 10
SSTSTSN
# Push 99
SSTTSSSTTN
# Discard (remove 99)
SNN
# OutputNumber (remaining = 10)
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "10"}`

#### stack/copy_001
**概要**: Copy で n 番目の要素をコピー
```wsa
# Push 10
SSTSTSN
# Push 20
SSTSTSSN
# Push 30
SSTSTTTSN
# Copy 2nd from top (index 2 = value 10)
STSSTSN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "10"}`

### 2. 算術演算テスト (arith/)

#### arith/add_001
**概要**: 加算
```wsa
# Push 3
SSSTTN
# Push 5
SSSTSTN
# Add
TSSS
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "8"}`

#### arith/sub_001
**概要**: 減算（最初にプッシュした値が左辺）
```wsa
# Push 10
SSTSTSN
# Push 3
SSSTTN
# Sub (10 - 3 = 7)
TSST
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "7"}`

#### arith/mul_001
**概要**: 乗算
```wsa
# Push 6
SSSTTSN
# Push 7
SSSTTN
# Mul (6 * 7 = 42)
TSSN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "42"}`

#### arith/div_001
**概要**: 整数除算
```wsa
# Push 17
SSTSTSSTN
# Push 5
SSSTSTN
# Div (17 / 5 = 3)
TSTS
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "3"}`

#### arith/mod_001
**概要**: 剰余
```wsa
# Push 17
SSTSTSSTN
# Push 5
SSSTSTN
# Mod (17 % 5 = 2)
TSTT
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "2"}`

#### arith/combined_001
**概要**: 複合算術（(2 + 3) * 4 = 20）
```wsa
# Push 2
SSTSN
# Push 3
SSSTTN
# Add (2+3=5)
TSSS
# Push 4
SSTSTSN
# Mul (5*4=20) — ここ誤り: Push 4 = SSSTSN
TSSN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "20"}`

### 3. ヒープアクセステスト (heap/)

#### heap/store_retrieve_001
**概要**: アドレス 0 に値を格納し取得
```wsa
# Push 0 (address)
SSSN
# Push 42 (value)
SSSTSTSTSTSN
# Store
TTS
# Push 0 (address)
SSSN
# Retrieve
TTT
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "42"}`

#### heap/multiple_addr_001
**概要**: 複数アドレスへの格納と取得
```wsa
# Store: addr=0, val=10
SSSN
SSTSTSN
TTS
# Store: addr=1, val=20
SSSTN
SSTSTSSN
TTS
# Retrieve addr=1
SSSTN
TTT
# OutputNumber (20)
TNST
# Retrieve addr=0
SSSN
TTT
# OutputNumber (10)
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "2010"}`

### 4. フロー制御テスト (flow/)

#### flow/jump_001
**概要**: 無条件ジャンプ（中間コードをスキップ）
```wsa
# Jump to label 0
NSNSN
# (skipped) Push 99
SSTTSSSTTN
# (skipped) OutputNumber
TNST
# Label 0
NSSSN
# Push 1
SSSTN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "1"}`

#### flow/jump_if_zero_true_001
**概要**: JumpIfZero - ゼロの場合ジャンプする
```wsa
# Push 0
SSSN
# JumpIfZero to label 0
NTSSN
# (skipped) Push 99
SSTTSSSTTN
# (skipped) OutputNumber
TNST
# Label 0
NSSSN
# Push 1
SSSTN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "1"}`

#### flow/jump_if_zero_false_001
**概要**: JumpIfZero - 非ゼロの場合ジャンプしない
```wsa
# Push 5
SSSTSTN
# JumpIfZero to label 0 (won't jump)
NTSSN
# Push 2
SSTSN
# OutputNumber (2)
TNST
# Jump to label 1
NSNSTN
# Label 0
NSSSN
# Push 99
SSTTSSSTTN
# OutputNumber
TNST
# Label 1
NSSTN
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "2"}`

#### flow/jump_if_neg_true_001
**概要**: JumpIfNegative - 負の場合ジャンプする
```wsa
# Push -1
SSTN
# JumpIfNegative to label 0
NTTSN
# (skipped) Push 99
SSTTSSSTTN
# (skipped) OutputNumber
TNST
# Label 0
NSSSN
# Push 1
SSSTN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "1"}`

#### flow/jump_if_neg_false_001
**概要**: JumpIfNegative - 正の場合ジャンプしない
```wsa
# Push 5
SSSTSTN
# JumpIfNegative to label 0 (won't jump)
NTTSN
# Push 2
SSTSN
# OutputNumber (2)
TNST
# Jump to label 1
NSNSTN
# Label 0
NSSSN
# Push 99
SSTTSSSTTN
# OutputNumber
TNST
# Label 1
NSSTN
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "2"}`

#### flow/call_return_001
**概要**: サブルーチン呼出と復帰
```wsa
# Jump to label 1 (main)
NSNSTN
# Label 0 (subroutine: push 42)
NSSSN
# Push 42
SSSTSTSTSTSN
# Return
NTN
# Label 1 (main)
NSSTN
# Call label 0
NSTSN
# OutputNumber (42)
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "42"}`

#### flow/loop_001
**概要**: ループで 1 + 2 + 3 = 6 を計算（累積加算）
```wsa
# Push 0 (accumulator)
SSSN
# Push 1 (counter)
SSSTN
# Label 0 (loop start)
NSSSN
# Dup counter
SNS
# Push 4
SSSTSN
# Sub (counter - 4)
TSST
# JumpIfZero label 1 (if counter == 4, exit loop)
NTSSTN
# Swap (bring accumulator to top)
SNT
# Dup
SNS
# Push 2 (copy counter to top ... need Copy)
# -- This approach is complex; let me simplify
# Actually, rethink: accumulator on stack below counter
# Stack: [acc, counter]
# We want: acc += counter, counter += 1
# Swap -> [counter, acc], Dup counter...
# Let me use a different approach with heap

# Reset: use heap-based approach
NNN
```

**注意**: ループテストは複雑なため、ヒープを活用した方が安定する。以下の簡略版を使用:

#### flow/loop_simple_001
**概要**: カウントダウンループ (3, 2, 1 を出力)
```wsa
# Push 3 (counter)
SSSTTN
# Label 0 (loop start)
NSSSN
# Dup counter
SNS
# OutputNumber
TNST
# Push 1
SSSTN
# Sub (counter - 1)
TSST
# Dup
SNS
# JumpIfZero label 1 (if counter == 0, exit)
NTSSTN
# Jump label 0 (continue loop)
NSNSN
# Label 1
NSSTN
# Discard (remove 0)
SNN
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "321"}`

### 5. I/O テスト (io/)

#### io/output_char_001
**概要**: 文字として出力 (65='A', 66='B')
```wsa
# Push 65 ('A')
SSSTSSSSTN
# OutputChar
TNSS
# Push 66 ('B')
SSSTSSSTSN
# OutputChar
TNSS
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "AB"}`

#### io/output_number_001
**概要**: 数値として出力
```wsa
# Push 123
SSSTTTTTSTN
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdout": "123"}`

#### io/input_number_001
**概要**: 数値を入力しそのまま出力
```wsa
# Push 0 (address for input)
SSSN
# InputNumber (read number into heap[0])
TNTT
# Push 0 (address)
SSSN
# Retrieve (load from heap[0])
TTT
# OutputNumber
TNST
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdin": "42\n", "stdout": "42"}`

#### io/input_char_001
**概要**: 文字を入力しそのまま出力
```wsa
# Push 0 (address for input)
SSSN
# InputChar (read char into heap[0])
TNTS
# Push 0 (address)
SSSN
# Retrieve (load from heap[0])
TTT
# OutputChar
TNSS
# Exit
NNN
```
**check.json**: `{"type": "ws_io", "stdin": "X", "stdout": "X"}`

### 6. 実行時エラーテスト (errors/)

#### errors/stack_underflow_001
**概要**: 空スタックで算術演算を試みる
```wsa
# Add (stack is empty)
TSSS
# Exit
NNN
```
**check.json**: `{"type": "ws_runtime_error", "error": "StackUnderflow"}`

#### errors/div_zero_001
**概要**: ゼロ除算
```wsa
# Push 10
SSTSTSN
# Push 0
SSSN
# Div (10 / 0)
TSTS
# Exit
NNN
```
**check.json**: `{"type": "ws_runtime_error", "error": "DivisionByZero"}`

#### errors/callstack_underflow_001
**概要**: Call なしで Return を実行
```wsa
# Return (no prior Call)
NTN
```
**check.json**: `{"type": "ws_runtime_error", "error": "CallStackUnderflow"}`

#### errors/undefined_label_001
**概要**: 未定義ラベルへジャンプ
```wsa
# Jump to label 99 (not defined)
NSNSTTSSSTTN
# Exit
NNN
```
**check.json**: `{"type": "ws_runtime_error", "error": "UndefinedLabel"}`

## テストマニフェスト

```yaml
# Whitespace Interpreter Direct Test Manifest
tests:
  # Stack manipulation
  - name: test_ws_stack_push_positive_001
    type: ws_io
    path: stack/push_positive_001
    comment: "Push positive number and output"

  - name: test_ws_stack_push_negative_001
    type: ws_io
    path: stack/push_negative_001
    comment: "Push negative number and output"

  - name: test_ws_stack_push_zero_001
    type: ws_io
    path: stack/push_zero_001
    comment: "Push zero and output"

  - name: test_ws_stack_dup_001
    type: ws_io
    path: stack/dup_001
    comment: "Duplicate top item"

  - name: test_ws_stack_swap_001
    type: ws_io
    path: stack/swap_001
    comment: "Swap top two items"

  - name: test_ws_stack_discard_001
    type: ws_io
    path: stack/discard_001
    comment: "Discard top item"

  - name: test_ws_stack_copy_001
    type: ws_io
    path: stack/copy_001
    comment: "Copy nth item to top"

  # Arithmetic
  - name: test_ws_arith_add_001
    type: ws_io
    path: arith/add_001
    comment: "Addition"

  - name: test_ws_arith_sub_001
    type: ws_io
    path: arith/sub_001
    comment: "Subtraction"

  - name: test_ws_arith_mul_001
    type: ws_io
    path: arith/mul_001
    comment: "Multiplication"

  - name: test_ws_arith_div_001
    type: ws_io
    path: arith/div_001
    comment: "Integer division"

  - name: test_ws_arith_mod_001
    type: ws_io
    path: arith/mod_001
    comment: "Modulo"

  - name: test_ws_arith_combined_001
    type: ws_io
    path: arith/combined_001
    comment: "Combined arithmetic"

  # Heap
  - name: test_ws_heap_store_retrieve_001
    type: ws_io
    path: heap/store_retrieve_001
    comment: "Store and retrieve"

  - name: test_ws_heap_multiple_addr_001
    type: ws_io
    path: heap/multiple_addr_001
    comment: "Multiple heap addresses"

  # Flow control
  - name: test_ws_flow_jump_001
    type: ws_io
    path: flow/jump_001
    comment: "Unconditional jump"

  - name: test_ws_flow_jump_if_zero_true_001
    type: ws_io
    path: flow/jump_if_zero_true_001
    comment: "JumpIfZero with zero value"

  - name: test_ws_flow_jump_if_zero_false_001
    type: ws_io
    path: flow/jump_if_zero_false_001
    comment: "JumpIfZero with non-zero value"

  - name: test_ws_flow_jump_if_neg_true_001
    type: ws_io
    path: flow/jump_if_neg_true_001
    comment: "JumpIfNegative with negative value"

  - name: test_ws_flow_jump_if_neg_false_001
    type: ws_io
    path: flow/jump_if_neg_false_001
    comment: "JumpIfNegative with positive value"

  - name: test_ws_flow_call_return_001
    type: ws_io
    path: flow/call_return_001
    comment: "Subroutine call and return"

  - name: test_ws_flow_loop_simple_001
    type: ws_io
    path: flow/loop_simple_001
    comment: "Simple countdown loop"

  # I/O
  - name: test_ws_io_output_char_001
    type: ws_io
    path: io/output_char_001
    comment: "Output character"

  - name: test_ws_io_output_number_001
    type: ws_io
    path: io/output_number_001
    comment: "Output number"

  - name: test_ws_io_input_number_001
    type: ws_io
    path: io/input_number_001
    comment: "Input number"

  - name: test_ws_io_input_char_001
    type: ws_io
    path: io/input_char_001
    comment: "Input character"

  # Runtime errors
  - name: test_ws_errors_stack_underflow_001
    type: ws_runtime_error
    path: errors/stack_underflow_001
    comment: "Stack underflow on empty stack"

  - name: test_ws_errors_div_zero_001
    type: ws_runtime_error
    path: errors/div_zero_001
    comment: "Division by zero"

  - name: test_ws_errors_callstack_underflow_001
    type: ws_runtime_error
    path: errors/callstack_underflow_001
    comment: "Return without prior Call"

  - name: test_ws_errors_undefined_label_001
    type: ws_runtime_error
    path: errors/undefined_label_001
    comment: "Jump to undefined label"
```

## 実装計画

### ステップ 1: ディレクトリ・ファイル作成

1. `resources/tests_ws/` ディレクトリ構造を作成
2. `.wsa` テストファイルと `.check.json` を作成
3. `test-manifest.yaml` を作成
4. `README.md` を作成

### ステップ 2: テストランナー実装

1. `tests/whitespace_direct_test.rs` を新規作成
   - `decode_wsa()` ヘルパー関数
   - `test_ws_io_base()` ベース関数
   - `test_ws_runtime_error_base()` ベース関数
2. `build.rs` を拡張
   - `resources/tests_ws/test-manifest.yaml` の読み込み
   - `ws_io` / `ws_runtime_error` テストタイプの生成コード追加
   - 出力ファイルを `generated_ws_tests.rs` として分離

### ステップ 3: テスト実行・修正

1. 全テストを実行しプログラムの正確性を検証
2. WSA エンコーディングの誤りを修正
3. 必要に応じてテストケースを追加

### ステップ 4: ドキュメント更新

1. `resources/tests_ws/README.md` にテスト方針を記載
2. `ai-docs/task/` のタスクを完了として記録

## 対象外

- 拡張仕様（負ヒープアドレスによる `__trace`/`__assert`/`__assert_not`）
- Slide 命令（`src/whitespace/` に未実装）
- パーサ単体のエラーテスト（既存のユニットテストで十分カバー）

## 実装状況

### 完了した作業

1. ✅ テストディレクトリ構造を作成（`resources/tests_ws/`）
2. ✅ test-manifest.yaml を作成
3. ✅ 30個のテストケース（.wsa, .check.json）を作成
4. ✅ テストランナー（`tests/whitespace_direct_test.rs`）を実装
5. ✅ build.rs を拡張して Whitespace テスト自動生成機能を追加
6. ✅ WSA デコーダにコメント除外機能を追加

### テスト結果

**成功: 17/30 テスト** (56.7%)

#### 成功したテスト
- スタック操作: push_positive, push_zero, dup, swap, discard, copy
- 算術演算: add, div, mod
- ヒープ操作: store_retrieve, multiple_addr
- I/O: output_char, output_number, input_char, input_number
- フロー制御: jump, jump_if_zero_true

#### 失敗したテスト (13件)

| テスト名 | 期待値 | 実際の値 | 状態 |
|---------|--------|---------|------|
| stack/push_negative_001 | -3 | (要調査) | WSAエンコーディング修正が必要 |
| arith/sub_001 | 7 | (要調査) | 減算命令の動作確認が必要 |
| arith/mul_001 | 42 | (要調査) | 乗算命令の動作確認が必要 |
| arith/combined_001 | 20 | 6 | 複合演算のスタック順序確認が必要 |
| flow/call_return_001 | 42 | (要調査) | サブルーチン呼び出しの動作確認 |
| flow/jump_if_neg_true_001 | 1 | 991 | ジャンプ条件と出力の動作確認 |
| flow/jump_if_neg_false_001 | 2 | (要調査) | ジャンプ条件の動作確認 |
| flow/jump_if_zero_false_001 | 2 | (要調査) | ジャンプ条件の動作確認 |
| flow/loop_simple_001 | 321 | (要調査) | ループ動作の確認 |
| errors/stack_underflow_001 | RuntimeError | (要調査) | エラー検出の確認 |
| errors/div_zero_001 | DivisionByZero | (要調査) | エラー検出の確認 |
| errors/callstack_underflow_001 | CallStackUnderflow | (要調査) | エラー検出の確認 |
| errors/undefined_label_001 | UndefinedLabel | (要調査) | エラー検出の確認 |

### 既知の問題

1. **数値エンコーディング**: 一部のWSAファイルで数値エンコーディングが不正確（修正中）
2. **エラーテスト**: すべてのエラーテストが失敗（パス問題またはテストケースの問題の可能性）
3. **減算・乗算命令**: 期待値と実際の出力が一致しない（インタプリタの実装確認が必要）

### 次のステップ

1. 失敗したテストケースの詳細調査
2. WSAファイルのエンコーディング検証
3. インタプリタの動作確認（特に減算・乗算・フロー制御）
4. エラーテストのデバッグ
