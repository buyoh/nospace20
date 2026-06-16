# compiler_ws/alloc_runtime.rs 分割設計

## 完了ステータス

**作業完了** (2026-03-01)

### 実施内容

- `src/compiler_ws/alloc_runtime.rs` (1713行) をサブディレクトリ構成に分割した
- `generate_function_prologue` / `generate_function_epilogue` の共通実装を `generate_common_prologue` / `generate_common_epilogue` として `mod.rs` に抽出
- テストヘルパー (`AllocOp`, `run_alloc_free_sequence`) を `mod.rs` の `test_helpers` モジュールに共通化
- `src/compiler_ws/label.rs` のコメントを `alloc_runtime/fsba.rs` に更新

### 最終ファイル構成

```
src/compiler_ws/alloc_runtime/
├── mod.rs      (~210行) AllocRuntime trait + generate_common_prologue/epilogue + test_helpers
├── bump.rs     (~260行) BumpAllocRuntime + Bump テスト
└── fsba.rs     (~670行) FsbaFirstFitAllocRuntime + FSBA テスト
```

### テスト結果

`cargo test alloc_runtime` : 26 tests, 0 failed  
`cargo test` (全体): 全テスト合格

---

## 現状

[src/compiler_ws/alloc_runtime.rs](../../../src/compiler_ws/alloc_runtime.rs) は 1713 行で、以下の構成:

| セクション | 行範囲 | 行数 | 内容 |
|------------|--------|------|------|
| trait 定義 | L1–49 | 49 | `AllocRuntime` trait (4 メソッド) |
| Bump 実装 | L51–212 | 162 | `BumpAllocRuntime` の trait impl |
| FSBA ラベル・定数 | L215–310 | 96 | ラベル定数モジュール + サイズクラス定義 |
| FSBA 固有メソッド | L312–862 | 551 | `generate_rt_alloc`, `generate_rt_free` 等 |
| FSBA trait impl | L864–970 | 107 | `AllocRuntime` trait 実装 |
| テスト | L972–1713 | **742** | 全体の **43%** がテスト |

## `AllocRuntime` trait 定義

```rust
pub trait AllocRuntime {
    fn generate_memory_init(&self, ctx: &CodeGenContext) -> WsProgram;
    fn generate_subroutines(&self, ctx: &CodeGenContext) -> WsProgram;
    fn generate_function_prologue(&self, ctx: &CodeGenContext, local_size: i64) -> WsProgram;
    fn generate_function_epilogue(&self, ctx: &CodeGenContext) -> WsProgram;
}
```

## 重複の分析

### prologue/epilogue の重複

`BumpAllocRuntime` と `FsbaFirstFitAllocRuntime` の `generate_function_prologue` / `generate_function_epilogue` がほぼ同一。FSBA 側のコメントに「BumpAllocRuntime と同じフロー」と明記されている。

| メソッド | Bump | FSBA | 重複度 |
|----------|------|------|--------|
| `generate_memory_init` | L57–75 | L865–903 | 異なる |
| `generate_subroutines` | L77–123 | L905–909 | 異なる（FSBA は委譲） |
| `generate_function_prologue` | L125–190 | L911–947 | **ほぼ同一** |
| `generate_function_epilogue` | L192–212 | L949–970 | **ほぼ同一** |

#### 改善案

共通の prologue/epilogue をデフォルト実装またはヘルパー関数として抽出:

```rust
/// prologue/epilogue の共通実装
fn generate_common_prologue(ctx: &CodeGenContext, local_size: i64) -> WsProgram {
    // Bump と FSBA で共有されるスタックフレームセットアップ
    // ...
}

fn generate_common_epilogue(ctx: &CodeGenContext) -> WsProgram {
    // Bump と FSBA で共有されるスタックフレームクリーンアップ
    // ...
}
```

**削減見込み**: ~60 行

## 分割方針

### ファイル構成案

```
src/compiler_ws/
├── alloc_runtime.rs         # AllocRuntime trait + 共通ヘルパー + ファクトリ
├── alloc_runtime_bump.rs    # BumpAllocRuntime 実装
├── alloc_runtime_fsba.rs    # FsbaFirstFitAllocRuntime 実装
└── (テストは各ファイル内 #[cfg(test)] に分散)
```

### 各ファイルの詳細

#### alloc_runtime.rs (~100 行)

```rust
mod alloc_runtime_bump;
mod alloc_runtime_fsba;

pub use alloc_runtime_bump::BumpAllocRuntime;
pub use alloc_runtime_fsba::FsbaFirstFitAllocRuntime;

/// メモリアロケータ抽象
pub trait AllocRuntime {
    fn generate_memory_init(&self, ctx: &CodeGenContext) -> WsProgram;
    fn generate_subroutines(&self, ctx: &CodeGenContext) -> WsProgram;
    fn generate_function_prologue(&self, ctx: &CodeGenContext, local_size: i64) -> WsProgram;
    fn generate_function_epilogue(&self, ctx: &CodeGenContext) -> WsProgram;
}

/// prologue/epilogue の共通実装（Bump と FSBA で共有）
pub(super) fn generate_common_prologue(ctx: &CodeGenContext, local_size: i64) -> WsProgram {
    // ...
}

pub(super) fn generate_common_epilogue(ctx: &CodeGenContext) -> WsProgram {
    // ...
}
```

#### alloc_runtime_bump.rs (~230 行)

| セクション | 行数 | 内容 |
|------------|------|------|
| 実装コード | ~100 | Bump trait impl (`memory_init`, `subroutines` + 共通関数委譲) |
| テスト | ~130 | Bump 固有テスト (L975–L1203) |

#### alloc_runtime_fsba.rs (~800 行)

| セクション | 行数 | 内容 |
|------------|------|------|
| ラベル・定数 | ~96 | `fsba_labels` モジュール + `FSBA_SIZE_CLASSES` |
| 固有メソッド | ~550 | `generate_rt_alloc`, `generate_general_alloc`, `generate_rt_free` 等 |
| trait impl | ~50 | `AllocRuntime` 実装（共通関数委譲） |
| テスト | ~500 | FSBA テスト (L1207–L1713) + テストヘルパー |

FSBA のテストが ~500 行と大きいが、テストヘルパー (`AllocOp`, `run_alloc_free_sequence`) は Bump テストからも使用されているため、共通ヘルパーとしての配置を検討:

```rust
// alloc_runtime.rs 内に #[cfg(test)] で共通テストヘルパーを配置
#[cfg(test)]
pub(super) mod test_helpers {
    pub enum AllocOp { ... }
    pub fn run_alloc_free_sequence(...) -> ... { ... }
}
```

### 代替案: サブディレクトリ化

ファイル数が増える場合はサブディレクトリも検討:

```
src/compiler_ws/alloc_runtime/
├── mod.rs      # trait + 共通ヘルパー + テストヘルパー
├── bump.rs     # BumpAllocRuntime
└── fsba.rs     # FsbaFirstFitAllocRuntime
```

ただし `compiler_ws` 内に既に 13 ファイルあり、サブディレクトリの方が整理しやすい。

## 行数変化予測

| ファイル | 現在 | 分割後 | 備考 |
|----------|------|--------|------|
| alloc_runtime.rs | 1713 | — | 分割 |
| alloc_runtime/mod.rs | — | ~100 | trait + 共通ヘルパー |
| alloc_runtime/bump.rs | — | ~230 | Bump + テスト |
| alloc_runtime/fsba.rs | — | ~800 | FSBA + テスト |
| **合計** | 1713 | ~1130 | ~580 行削減（重複解消 + 構造改善） |

実際には行数削減よりも、各ファイルが単一の実装に集中することで**認知負荷が軽減**される効果が大きい。

## 推奨実行順序

1. **prologue/epilogue 共通関数の抽出** — リファクタリングの前準備
2. **ファイル分割** — 境界が明確なため機械的に実施可能
3. **テストヘルパーの共通化** — Bump/FSBA テストの共通部分を整理

## テストへの影響

- テストコードの移動のみ（ロジック変更なし）
- `#[cfg(test)]` モジュールの参照パスが変わるが、テスト内容は不変
- `cargo test` で全テスト通過を確認

## リスク

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| 共通 prologue/epilogue 抽出時の微妙な差異見落とし | 低 | diff で厳密に比較してから共通化 |
| テストヘルパーのモジュール間参照 | 低 | `pub(super)` で制御 |
| 変更頻度が低いため費用対効果 | — | 他タスクとの並行実施で工数を吸収 |
