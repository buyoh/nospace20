# プロファイラ設計

## ProfileStats 構造体

`src/whitespace/profiler.rs` に配置。

```rust
/// Whitespace VM のプロファイリング統計
#[derive(Debug, Clone, Default)]
pub struct ProfileStats {
    // === 命令カウント ===
    /// 命令種別ごとの実行回数
    pub instruction_counts: InstructionCounts,

    // === メモリアクセス ===
    /// ヒープ Store のアクセス範囲
    pub heap_store_min: Option<i64>,
    pub heap_store_max: Option<i64>,
    pub heap_store_count: usize,
    /// ヒープ Retrieve のアクセス範囲
    pub heap_retrieve_min: Option<i64>,
    pub heap_retrieve_max: Option<i64>,
    pub heap_retrieve_count: usize,
    /// ヒープに Store されたユニークアドレス数
    pub heap_unique_addresses: usize,

    // === スタック ===
    /// データスタックの最大深度
    pub stack_max_depth: usize,
    /// コールスタックの最大深度
    pub call_stack_max_depth: usize,
}

/// 命令種別ごとの実行回数
#[derive(Debug, Clone, Default)]
pub struct InstructionCounts {
    // スタック操作
    pub push: usize,
    pub duplicate: usize,
    pub copy: usize,
    pub swap: usize,
    pub discard: usize,
    // 算術演算
    pub add: usize,
    pub sub: usize,
    pub mul: usize,
    pub div: usize,
    pub modulo: usize,
    // ヒープアクセス
    pub store: usize,
    pub retrieve: usize,
    // フロー制御
    pub label: usize,
    pub call: usize,
    pub jump: usize,
    pub jump_if_zero: usize,
    pub jump_if_negative: usize,
    pub return_: usize,
    pub exit: usize,
    // I/O
    pub output_char: usize,
    pub output_number: usize,
    pub input_char: usize,
    pub input_number: usize,
}
```

## VM への統合

### WhitespaceVM への追加フィールド

```rust
pub struct WhitespaceVM {
    // ... 既存フィールド ...

    // === プロファイリング ===
    /// プロファイリングが有効か
    profiling: bool,
    /// プロファイリング統計（profiling=true 時のみ更新）
    profile_stats: ProfileStats,
}
```

### ビルダーメソッド

```rust
impl WhitespaceVM {
    /// プロファイリングモードを有効にして構築
    pub fn with_profiling(mut self, enabled: bool) -> Self {
        self.profiling = enabled;
        self
    }

    /// プロファイリング統計を取得
    pub fn profile_stats(&self) -> &ProfileStats {
        &self.profile_stats
    }
}
```

### execute_instruction 内のフック

`execute_instruction` の各命令分岐で、`self.profiling` が true の場合にカウンタをインクリメントする。

パフォーマンスへの影響を最小化するため、各命令の先頭で単純な `if self.profiling { ... }` チェックのみ行う。分岐予測によりほぼゼロオーバーヘッド。

```rust
// 例: Store 命令
Instruction::Store => {
    let val = self.stack_pop()?;
    let addr = self.stack_pop()?;
    if self.profiling {
        self.profile_stats.instruction_counts.store += 1;
        self.profile_stats.heap_store_count += 1;
        self.profile_stats.heap_store_min = Some(
            self.profile_stats.heap_store_min.map_or(addr, |m| m.min(addr))
        );
        self.profile_stats.heap_store_max = Some(
            self.profile_stats.heap_store_max.map_or(addr, |m| m.max(addr))
        );
    }
    self.heap_store(addr, val)?;
    self.pc += 1;
}
```

### step() 内のデータスタック深度追跡

`step()` メソッドのループ内で、各命令実行後にスタック深度を記録:

```rust
if self.profiling {
    let depth = self.data_stack.len();
    if depth > self.profile_stats.stack_max_depth {
        self.profile_stats.stack_max_depth = depth;
    }
    let call_depth = self.call_stack.len();
    if call_depth > self.profile_stats.call_stack_max_depth {
        self.profile_stats.call_stack_max_depth = call_depth;
    }
}
```

### ユニークアドレス数の計算

実行中にユニークアドレスを逐一追跡するのはオーバーヘッドが大きいため、実行完了後に `self.heap.len()` から取得する。`profile_stats()` 呼び出し時に計算。

## ProfileStats のサマリメソッド

```rust
impl ProfileStats {
    /// 総命令実行数
    pub fn total_instructions(&self) -> usize {
        let c = &self.instruction_counts;
        c.push + c.duplicate + c.copy + c.swap + c.discard
        + c.add + c.sub + c.mul + c.div + c.modulo
        + c.store + c.retrieve
        + c.label + c.call + c.jump + c.jump_if_zero + c.jump_if_negative + c.return_ + c.exit
        + c.output_char + c.output_number + c.input_char + c.input_number
    }

    /// アクセスされたメモリ範囲（Store + Retrieve の全体）
    pub fn memory_range(&self) -> Option<(i64, i64)> {
        let min = match (self.heap_store_min, self.heap_retrieve_min) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let max = match (self.heap_store_max, self.heap_retrieve_max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        min.zip(max)
    }
}
```

## ファイル配置

- `src/whitespace/profiler.rs`: `ProfileStats`, `InstructionCounts` 構造体
- `src/whitespace/mod.rs`: `pub mod profiler;` 追加、re-export
- `src/whitespace/interpreter.rs`: プロファイリングフィールド追加、`execute_instruction` 修正
