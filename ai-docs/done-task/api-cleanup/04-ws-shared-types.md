# Whitespace 共有型の独立モジュール化

日付: 2026-03-01

## 概要

`Instruction`, `WsNumber`, `WsChar`, `LabelId` 等の Whitespace 言語の基本型が `compiler_ws` モジュール内に定義されているが、これらは `whitespace`（パーサ + VM）モジュールからも使用される共有型である。`whitespace` が `compiler_ws` の内部型を参照する逆方向の依存を解消するため、共有型を独立モジュールに切り出す。

## 現状の依存構造

```
compiler_ws/
├── types.rs        ← WsChar, WsNumber, LabelId, HeapAddress 定義
├── instruction.rs  ← Instruction 定義（types.rs に依存）
├── label.rs        ← LabelAllocator + reserved_labels 定義
├── program.rs      ← WsProgram 定義（instruction.rs に依存）
└── ...

whitespace/
├── mod.rs          ← pub use crate::compiler_ws::{Instruction, LabelId, WsChar, WsNumber}
├── parser.rs       ← use crate::compiler_ws::{Instruction, LabelId, WsChar, WsNumber}
└── interpreter.rs  ← use crate::compiler_ws::{Instruction, LabelId}
```

`whitespace` → `compiler_ws` の参照は **Whitespace 言語の基本型のみ**であり、コンパイラのロジックには依存していない。

## 移動対象の型

### `compiler_ws/types.rs` から移動

| 型 | 定義 | 移動先 |
|----|------|--------|
| `WsChar` | `enum WsChar { Space, Tab, Lf }` | `base/ws_types.rs` |
| `WsNumber` | `struct WsNumber(pub i64)` + `encode()` メソッド | `base/ws_types.rs` |
| `LabelId` | `struct LabelId(pub u32)` + `to_ws_value()`, `offset()` メソッド | `base/ws_types.rs` |
| `HeapAddress` | `struct HeapAddress(pub i64)` + `value()`, `offset()` メソッド | `base/ws_types.rs` |

### `compiler_ws/instruction.rs` から移動

| 型 | 定義 | 移動先 |
|----|------|--------|
| `Instruction` | 全 20 命令の enum + `encode()`, `to_mnemonic()` | `base/ws_types.rs` |

### `compiler_ws/program.rs` から移動

| 型 | 定義 | 移動先 |
|----|------|--------|
| `WsProgram` | `struct WsProgram { instructions: Vec<Instruction> }` | `base/ws_types.rs` |

### `compiler_ws/label.rs` の扱い

`LabelAllocator` と `reserved_labels` モジュールは **コンパイラ固有**のロジックであり、`compiler_ws` に残す。`LabelId` 型のみ移動。

## 移動しない型

| 型 | 理由 |
|----|------|
| `LabelAllocator` | コンパイラ固有のラベル管理ロジック |
| `reserved_labels` | コンパイラ固有の予約ラベル定数 |

## 改善後の依存構造

```
base/
├── mod.rs
├── ws_types.rs     ← WsChar, WsNumber, LabelId, HeapAddress, Instruction, WsProgram
├── location.rs     ← SourceLocation (既存)
└── ...

compiler_ws/        ← base::ws_types を使用, whitespace に非依存
├── label.rs        ← LabelAllocator, reserved_labels (LabelId は base から import)
├── ...

whitespace/         ← base::ws_types を使用, compiler_ws に非依存
├── mod.rs          ← pub use crate::base::ws_types::{...}
├── parser.rs
└── interpreter.rs
```

## 作業ステップ

### Step 1: `src/base/ws_types.rs` の作成

`compiler_ws/types.rs`, `compiler_ws/instruction.rs`, `compiler_ws/program.rs` から型定義とそのメソッド実装を `base/ws_types.rs` に移動。

```rust
// src/base/ws_types.rs

/// Whitespace の基本文字（空白・タブ・改行）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsChar { Space, Tab, Lf }

/// Whitespace の数値表現
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsNumber(pub i64);

impl WsNumber {
    pub fn encode(&self) -> Vec<WsChar> { ... }
}

/// Whitespace のラベル識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelId(pub u32);

impl LabelId {
    pub fn to_ws_value(&self) -> WsNumber { ... }
    pub fn offset(&self, n: u32) -> LabelId { ... }
}

/// Whitespace のヒープアドレス
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapAddress(pub i64);

impl HeapAddress {
    pub fn new(addr: i64) -> Self { ... }
    pub fn value(&self) -> i64 { ... }
    pub fn offset(&self, n: i64) -> HeapAddress { ... }
}

/// Whitespace の命令
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Push(WsNumber), Duplicate, Copy(WsNumber), Swap, Discard,
    Add, Sub, Mul, Div, Mod,
    Store, Retrieve,
    Label(LabelId), Call(LabelId), Jump(LabelId),
    JumpIfZero(LabelId), JumpIfNegative(LabelId),
    Return, Exit,
    OutputChar, OutputNumber, InputChar, InputNumber,
}

impl Instruction {
    pub fn encode(&self) -> Vec<WsChar> { ... }
    pub fn to_mnemonic(&self) -> String { ... }
}

/// Whitespace プログラム（命令列のコンテナ）
pub struct WsProgram {
    instructions: Vec<Instruction>,
}

impl WsProgram {
    pub fn new() -> Self { ... }
    pub fn push(&mut self, inst: Instruction) { ... }
    pub fn to_whitespace(&self) -> String { ... }
    pub fn to_debug_string(&self) -> String { ... }
    pub fn into_instructions(self) -> Vec<Instruction> { ... }
    pub fn from_instructions(insts: Vec<Instruction>) -> Self { ... }
    pub fn instructions(&self) -> &[Instruction] { ... }
}
```

### Step 2: `src/base/mod.rs` の更新

```rust
pub mod ws_types;
```

### Step 3: `compiler_ws` 内の import パス更新

`compiler_ws/` 内の全ファイルで:
```rust
// Before
use super::types::{WsChar, WsNumber, LabelId, HeapAddress};
use super::instruction::Instruction;

// After
use crate::base::ws_types::{WsChar, WsNumber, LabelId, HeapAddress, Instruction};
```

既存の `compiler_ws/types.rs`, `compiler_ws/instruction.rs`, `compiler_ws/program.rs` は削除し、`compiler_ws/mod.rs` で `base::ws_types` を re-export:

```rust
// src/compiler_ws/mod.rs
pub use crate::base::ws_types::{
    HeapAddress, Instruction, LabelId, WsChar, WsNumber, WsProgram,
};
```

### Step 4: `whitespace` 内の import パス更新

```rust
// Before
use crate::compiler_ws::instruction::Instruction;
use crate::compiler_ws::types::{LabelId, WsChar, WsNumber};

// After
use crate::base::ws_types::{Instruction, LabelId, WsChar, WsNumber};
```

`whitespace/mod.rs` の re-export も更新:
```rust
pub use crate::base::ws_types::{Instruction, LabelId, WsChar, WsNumber};
```

### Step 5: `lib.rs` の更新

`compiler_ws` のサブモジュール公開範囲を見直す。`instruction`, `types`, `program` サブモジュールは不要になるため、`compiler_ws` の `pub mod` を整理。

### Step 6: テスト確認

```bash
cargo test
cargo build --features wasm --target wasm32-unknown-unknown
```

## 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/base/mod.rs` | `pub mod ws_types` 追加 |
| `src/base/ws_types.rs` | **新規作成** |
| `src/compiler_ws/types.rs` | 削除（内容は `base/ws_types.rs` に移動） |
| `src/compiler_ws/instruction.rs` | 削除（内容は `base/ws_types.rs` に移動） |
| `src/compiler_ws/program.rs` | 削除（内容は `base/ws_types.rs` に移動） |
| `src/compiler_ws/mod.rs` | re-export 追加、サブモジュール宣言更新 |
| `src/compiler_ws/` 全 `.rs` | import パス更新 |
| `src/whitespace/mod.rs` | re-export パス更新 |
| `src/whitespace/parser.rs` | import パス更新 |
| `src/whitespace/interpreter.rs` | import パス更新 |
| `src/lib.rs` | re-export パス更新 |

## 作業見積もり

中。型定義とメソッド実装のファイル間移動が主だが、import パス更新の影響範囲が広い。ロジックの変更はなし。

## 備考

- `compiler_ws` が `base::ws_types` を re-export することで、外部から `compiler_ws::Instruction` 等でアクセスしていたコードとの後方互換性を維持できる
- `LabelAllocator` は `LabelId` 型に依存するが、`LabelId` のみ移動するため `use crate::base::ws_types::LabelId` で解決
- `compiler_ws/types.rs` に `HeapAddress` が含まれているが、これは `compiler_ws` 内でしか使われない可能性もある。ただし Whitespace のヒープアドレスという概念は共有的なため、一緒に移動する
