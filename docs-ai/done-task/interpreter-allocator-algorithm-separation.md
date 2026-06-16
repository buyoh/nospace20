# `src/algorithm/` モジュール分離設計

## 目的

WS コンパイラとインタプリタが同一のアロケータアルゴリズムを使用するにあたり、
アルゴリズムの定数・仕様を `src/algorithm/` モジュールに集約する。

### 動機

- WS コンパイラはアルゴリズムを **Whitespace 命令列として出力** する
- インタプリタはアルゴリズムを **Rust コードで直接実行** する
- 実装コードの共有は構造上不可能
- しかし、アルゴリズムの **パラメータ（定数）** と **仕様（分類ロジック）** を一元管理しないと:
  - 定数のずれ（サイズクラスの変更忘れ等）が発生し得る
  - アルゴリズム検証が両モジュールに分散する
  - 変更時に二箇所を同時に修正する必要があり、不整合のリスクがある

## 分析: 共有可能な要素と不可能な要素

### 共有可能

| 要素 | 現在の場所 | 内容 |
|------|-----------|------|
| FSBA サイズクラス（ブロックサイズ） | `compiler_ws/alloc_runtime/fsba.rs` `FSBA_SIZE_CLASSES` | `[2, 4, 8, 16, 32]` |
| サイズクラス数 | `compiler_ws/memory.rs` `FSBA_CLASS_COUNT` | `5` |
| ブロックヘッダーサイズ | 暗黙的（コード内に `Push(1)` で散在） | `1` |
| 分割最小残余サイズ | 暗黙的（`Push(2); Sub; JumpIfNegative` で埋込み） | `2` |
| サイズクラス分類ロジック | `fsba.rs` のカスケード比較コード | `total <= block_size` で判定 |
| ユーザーサイズ→合計サイズ変換 | `fsba.rs` の `generate_rt_alloc` 冒頭 | `max(size + 1, 2)` |
| 分割可否判定 | `fsba.rs` の `GENERAL_ALLOC_FOUND` | `remainder >= 2` |

### 共有不可能

| 要素 | 理由 |
|------|------|
| alloc/free の実装コード | コンパイラは WS 命令を出力、インタプリタは Rust を実行 |
| ブロックデータ構造 | コンパイラはフラットヒープ上のセル列、インタプリタは `BTreeMap<i64, MemoryBlock>` |
| フリーリスト構造 | コンパイラはヒープ内 next ポインタ、インタプリタは Rust の `Vec` または構造体 |
| WS ラベル・制御フロー | コンパイラ固有 |
| ヒープレイアウト定数 | `ALLOC_FREE_HEAD`, `ALLOC_HEAP_TOP` 等は WS ヒープ固有のアドレス |

## 設計: `src/algorithm/alloc_spec.rs`

### モジュール構成

```
src/
  algorithm/
    mod.rs          -- pub mod alloc_spec;
    alloc_spec.rs   -- アロケータ仕様定数・関数
  lib.rs            -- pub mod algorithm; を追加
```

### `alloc_spec.rs` の内容

```rust
//! アロケータアルゴリズムの共通仕様
//!
//! WS コンパイラ (`compiler_ws::alloc_runtime`) と
//! インタプリタ (`interpreter::allocator`) の両方から参照される。
//! アルゴリズムのパラメータと分類ロジックを一元管理し、
//! 実装間の不整合を防ぐ。

/// FSBA サイズクラスのブロックサイズ（ヘッダー含む合計サイズ）
///
/// 各サイズクラスは固定サイズのフリーリストを持つ。
/// ユーザーリクエストの合計サイズ（ヘッダー込み）がこれ以下なら、
/// 対応するクラスの FSBA で確保される。
pub const FSBA_BLOCK_SIZES: [i64; FSBA_CLASS_COUNT] = [2, 4, 8, 16, 32];

/// FSBA サイズクラス数
pub const FSBA_CLASS_COUNT: usize = 5;

/// ブロックヘッダーサイズ
///
/// 各ブロックの先頭 1 セルにブロック合計サイズが格納される。
/// ユーザーがアクセスできるのはヘッダーの次のセル（ptr = block + 1）以降。
pub const HEADER_SIZE: i64 = 1;

/// 最小ブロックサイズ（ヘッダー含む合計）
///
/// フリーリストでは block[0]=size, block[1]=next_ptr を使うため、
/// 最小 2 セルが必要。
pub const MIN_BLOCK_SIZE: i64 = 2;

/// ブロック分割時の最小残余サイズ
///
/// General alloc でブロックを分割する際、残余がこの値未満なら分割せず
/// ブロック全体を使用する。
pub const SPLIT_MIN_REMAINDER: i64 = 2;

/// ユーザーリクエストサイズから必要な合計サイズ（ヘッダー含む）を計算する。
///
/// - ヘッダー分 (+1) を加算
/// - 最小ブロックサイズ (2) 未満にならないよう保証
///
/// WS コンパイラではこのロジックを WS 命令として出力する。
/// インタプリタではこの関数を直接呼び出す。
pub const fn total_from_user_size(user_size: i64) -> i64 {
    let total = user_size + HEADER_SIZE;
    if total < MIN_BLOCK_SIZE {
        MIN_BLOCK_SIZE
    } else {
        total
    }
}

/// 合計サイズが属する FSBA サイズクラスのインデックスを返す。
///
/// 合計サイズが最大クラス (`FSBA_BLOCK_SIZES[FSBA_CLASS_COUNT-1]`) を超える場合は `None`。
/// `None` の場合、呼び出し元は汎用アロケータ（First-Fit + バンプ）を使う。
pub fn fsba_class_for(total_size: i64) -> Option<usize> {
    FSBA_BLOCK_SIZES
        .iter()
        .position(|&block_size| total_size <= block_size)
}

/// ブロック分割が可能かを判定する。
///
/// General alloc で見つかったブロックの合計サイズが `block_total_size` で、
/// 要求された合計サイズが `requested_total_size` のとき、
/// 残余 (`block_total_size - requested_total_size`) が `SPLIT_MIN_REMAINDER` 以上なら
/// ブロックを分割できる。
pub const fn can_split(block_total_size: i64, requested_total_size: i64) -> bool {
    block_total_size - requested_total_size >= SPLIT_MIN_REMAINDER
}
```

## 移行計画

### Phase 0 で行う変更

#### 1. `src/algorithm/` モジュール新規作成

- `src/algorithm/mod.rs` — `pub mod alloc_spec;`
- `src/algorithm/alloc_spec.rs` — 上記の定数・関数

#### 2. `src/lib.rs` にモジュール登録

```rust
pub mod algorithm;
```

#### 3. `compiler_ws/alloc_runtime/fsba.rs` のリファクタリング

**Before:**
```rust
const FSBA_SIZE_CLASSES: [(i64, i64, i64); 5] = [
    (0, 2, 3),
    (1, 4, 5),
    (2, 8, 9),
    (3, 16, 17),
    (4, 32, 33),
];
```

**After:**
```rust
use crate::algorithm::alloc_spec;

/// FSBA サイズクラスカスケード用テーブル: (class_index, block_size, cascade_threshold)
///
/// `alloc_spec::FSBA_BLOCK_SIZES` から生成。
/// cascade_threshold = block_size + 1 (WS の `jn` で `total <= block_size` を判定するため)
const FSBA_SIZE_CLASSES: [(i64, i64, i64); alloc_spec::FSBA_CLASS_COUNT] = {
    let bs = alloc_spec::FSBA_BLOCK_SIZES;
    [
        (0, bs[0], bs[0] + 1),
        (1, bs[1], bs[1] + 1),
        (2, bs[2], bs[2] + 1),
        (3, bs[3], bs[3] + 1),
        (4, bs[4], bs[4] + 1),
    ]
};
```

要点:
- `FSBA_SIZE_CLASSES` の `block_size` を `alloc_spec::FSBA_BLOCK_SIZES` から導出
- `cascade_threshold` は WS 固有（`jn` 命令によるサイズ判定）なのでここで計算
- `class_index` もここでの列挙順ベースなのでそのまま

#### 4. `compiler_ws/memory.rs` のリファクタリング

**Before:**
```rust
pub const FSBA_CLASS_COUNT: i64 = 5;
```

**After:**
```rust
pub const FSBA_CLASS_COUNT: i64 = crate::algorithm::alloc_spec::FSBA_CLASS_COUNT as i64;
```

#### 5. `fsba.rs` 内の他の暗黙定数の置き換え

`generate_rt_alloc` 内:
- `Push(WsNumber(1))` (ヘッダーサイズ) → `Push(WsNumber(alloc_spec::HEADER_SIZE))`
- `Push(WsNumber(2))` (最小ブロックサイズ) → `Push(WsNumber(alloc_spec::MIN_BLOCK_SIZE))`

`generate_general_alloc` 内:
- `Push(WsNumber(2)); Sub; JumpIfNegative(...)` (分割判定) → `Push(WsNumber(alloc_spec::SPLIT_MIN_REMAINDER))`

`generate_rt_free` 内:
- `class_sizes: [i64; 5] = [2, 4, 8, 16, 32]` → `alloc_spec::FSBA_BLOCK_SIZES`

#### 6. テスト全パス確認

リファクタリングのみ（動作変更なし）のため、既存テストがすべてパスすることを確認。

## インタプリタ側での利用（Phase 1 以降）

`src/interpreter/allocator.rs` は `alloc_spec` の定数・関数を使用して
FSBA + First-Fit アルゴリズムを Rust で直接実装する。

```rust
use crate::algorithm::alloc_spec;

impl InterpreterAllocator {
    pub fn alloc(&mut self, user_size: i64) -> i64 {
        let total = alloc_spec::total_from_user_size(user_size);
        match alloc_spec::fsba_class_for(total) {
            Some(class) => self.fsba_alloc(class),
            None => self.general_alloc(total),
        }
    }

    pub fn free(&mut self, ptr: i64) {
        let block = /* ptr からブロックを特定 */;
        let block_size = /* ブロックの合計サイズ */;
        match alloc_spec::fsba_class_for(block_size) {
            Some(class) => self.fsba_free(block, class),
            None => self.general_free(block),
        }
    }
}
```

詳細は [allocator-design.md](allocator-design.md) を参照。

## 変更の影響範囲

| 対象 | 変更内容 |
|------|---------|
| `src/algorithm/mod.rs` | **新規**: `pub mod alloc_spec;` |
| `src/algorithm/alloc_spec.rs` | **新規**: 定数・関数 |
| `src/lib.rs` | `pub mod algorithm;` 追加 |
| `src/compiler_ws/alloc_runtime/fsba.rs` | 定数を `alloc_spec` 参照に変更 |
| `src/compiler_ws/memory.rs` | `FSBA_CLASS_COUNT` を `alloc_spec` 参照に変更 |
| `src/interpreter/allocator.rs` | **新規** (Phase 1): `alloc_spec` を使用 |
