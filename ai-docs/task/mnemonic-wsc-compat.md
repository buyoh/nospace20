# ニーモニック出力を wsc 形式に近づける

## 概要

`--target mnemonic` で出力されるニーモニック表記を、外部ツール wsc (whitespacers) のアセンブリ形式に近づける。
wsc との互換性を高めることで、出力を直接 wsc に渡す等のワークフローを可能にする。

## wsc のアセンブリ形式

```
Stack manipulation - push INTEGER, dup, swap, copy INTEGER, pop, slide INTEGER
Arithmetic         - add, sub, mul, div, mod
Heap manipulation  - get, set
Control flow       - label LABEL, call LABEL, jmp LABEL, jz LABEL, jn LABEL, ret, exit
IO                 - pnum, pchr, inum, ichr
```

- ラベル宣言は `LABEL:` 構文もサポート
- コメントは `;` で記述
- 命令はインデントされ、ラベルは行頭

## 現状との差異

### 命令名の変更（7件）

| カテゴリ | 現在 | wsc | 変更内容 |
|---------|------|-----|---------|
| スタック | `discard` | `pop` | リネーム |
| ヒープ | `store` | `set` | リネーム |
| ヒープ | `retrieve` | `get` | リネーム |
| I/O | `printc` | `pchr` | リネーム |
| I/O | `printi` | `pnum` | リネーム |
| I/O | `readc` | `ichr` | リネーム |
| I/O | `readi` | `inum` | リネーム |

### フォーマットの変更

| 項目 | 現在 | wsc | 変更内容 |
|------|------|-----|---------|
| ラベル宣言 | `label_N`（行単体） | `LABEL:` | `label_N:` 形式に変更 |
| インデント | なし（全命令が行頭） | 命令はインデント、ラベルは行頭 | 命令に4スペースのインデントを追加 |

### 未実装命令（対象外）

| 命令 | 備考 |
|------|------|
| `slide INTEGER` | Whitespace 仕様上の命令だが、nospace コンパイラが生成しないため今回は対象外。必要になった時点で追加対応。 |

## 修正対象ファイル

### 1. `src/compiler_ws/instruction.rs` - `to_mnemonic()` メソッド

命令名のリネームとラベル宣言フォーマットの変更。

**変更前:**
```rust
Discard => "discard".to_string(),
Store => "store".to_string(),
Retrieve => "retrieve".to_string(),
Label(id) => format!("label_{}", id.0),
OutputChar => "printc".to_string(),
OutputNumber => "printi".to_string(),
InputChar => "readc".to_string(),
InputNumber => "readi".to_string(),
```

**変更後:**
```rust
Discard => "pop".to_string(),
Store => "set".to_string(),
Retrieve => "get".to_string(),
Label(id) => format!("label_{}:", id.0),
OutputChar => "pchr".to_string(),
OutputNumber => "pnum".to_string(),
InputChar => "ichr".to_string(),
InputNumber => "inum".to_string(),
```

### 2. `src/compiler_ws/program.rs` - `to_debug_string()` メソッド

ラベル以外の命令にインデントを追加。

**変更前:**
```rust
pub fn to_debug_string(&self) -> String {
    self.instructions
        .iter()
        .map(|inst| inst.to_mnemonic())
        .collect::<Vec<_>>()
        .join("\n")
}
```

**変更後:**
```rust
pub fn to_debug_string(&self) -> String {
    self.instructions
        .iter()
        .map(|inst| {
            if matches!(inst, Instruction::Label(_)) {
                inst.to_mnemonic()
            } else {
                format!("    {}", inst.to_mnemonic())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

### 3. テストへの影響

- `tests/compile_test.rs` - `"push"` と `"ret"` を含むか検証するのみ。インデント追加後も含まれるため変更不要。
- `tools/wasm-test/test.mjs` - `"push"` を含むか検証するのみ。変更不要。

## 出力例

**変更前:**
```
push 2
push 8
store
label_0
jmp label_17
label_16
push 42
printi
push 10
printc
ret
label_17
call label_16
exit
```

**変更後:**
```
    push 2
    push 8
    set
label_0:
    jmp label_17
label_16:
    push 42
    pnum
    push 10
    pchr
    ret
label_17:
    call label_16
    exit
```

## 作業ステップ

1. `src/compiler_ws/instruction.rs` の `to_mnemonic()` を修正（命令名リネーム + ラベル宣言フォーマット変更）
2. `src/compiler_ws/program.rs` の `to_debug_string()` にインデントロジック追加
3. `cargo test` で既存テストが通ることを確認
4. 手動確認: サンプルコードを mnemonic ターゲットでコンパイルし出力確認

## ステータス

- 未着手
