# compiler_ws モジュールへの変更設計

## 概要

`--std-ext alloc` 有効時に、コンパイラが生成する Whitespace コードを変更する。主な変更箇所は以下の 6 領域:

1. **memory.rs**: 新しい定数の追加
2. **context.rs**: `alloc_ext` フラグの追加
3. **alloc_runtime.rs**: アロケータランタイムのコード生成（新規モジュール、分離テスト可能）
4. **builtin.rs**: ヘッダー生成とフレーム管理の分岐、alloc_runtime の呼び出し
5. **statement.rs**: 関数定義のフレーム確保方式の分岐
6. **expression.rs**: `__alloc`/`__free` 組み込み関数のコード生成
7. **mod.rs**: `compile_with_options` への alloc_ext パラメータ追加

## Phase 1: `--std-ext alloc` 基盤整備

### 1.1 compile_property.rs

`TargetExtension` に `Alloc` バリアントを追加:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetExtension {
    Debug,
    Alloc,  // 新規
}
```

### 1.2 nospace20.rs (CLI)

`CliTargetExt` に `Alloc` を追加:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliTargetExt {
    Debug,
    Alloc,  // 新規
}
```

変換ロジックに `CliTargetExt::Alloc => TargetExtension::Alloc` を追加。

### 1.3 memory.rs

`FSBA_TABLE_PTR` 定数を追加:

```rust
impl MemoryLayout {
    pub const ALLOC_FREE_HEAD: HeapAddress = HeapAddress(5);
    pub const ALLOC_HEAP_TOP: HeapAddress = HeapAddress(6);
    pub const FSBA_TABLE_PTR: HeapAddress = HeapAddress(7);
}
```

### 1.4 context.rs

`CodeGenContext` に `alloc_ext: bool` フラグを追加:

```rust
pub struct CodeGenContext<'a> {
    // ... 既存フィールド ...
    debug_ext: bool,
    alloc_ext: bool,  // 新規
}
```

`new_with_options` のシグネチャを拡張。`--std-ext debug` のパターンに倣って `enter_function` 等で子コンテキストに伝播。

**設計選択**: `new_with_options` の引数が増えてきたため、オプション構造体の導入を検討:

```rust
pub struct CodeGenOptions {
    pub debug_ext: bool,
    pub alloc_ext: bool,
}

impl<'a> CodeGenContext<'a> {
    pub fn new_with_options(scope: &'a Scope, options: &CodeGenOptions) -> Self { ... }
}
```

### 1.5 mod.rs

```rust
pub fn compile_with_options(scope: &Scope, debug_ext: bool) -> Vec<WhitespaceInstruction> {
```

を以下のように変更（または新しいオーバーロードを追加）:

```rust
pub fn compile_with_options(scope: &Scope, debug_ext: bool, alloc_ext: bool) -> Vec<WhitespaceInstruction> {
```

### 1.6 lib.rs

公開 API に alloc 対応版を追加:

```rust
pub fn compile_to_whitespace_with_options(scope: &Scope, debug_ext: bool, alloc_ext: bool) -> String { ... }
```

## Phase 2: アロケータサブルーチンのコード生成

アロケータは二層アーキテクチャで実装する（詳細は [fixed-size-block-allocator.md](fixed-size-block-allocator.md) 参照）:
- **第 1 層**: 固定サイズブロックアロケータ (FSBA) — サイズクラス [2, 4, 8, 16, 32] 毎のフリーリストで O(1) alloc/free
- **第 2 層**: 汎用アロケータ (First-Fit + バンプ) — >32 セルの大きなブロック用

### 2.1 alloc_runtime.rs（新規モジュール）

アロケータランタイムの WS コード生成を **独立したモジュール** として実装する。`CodeGenContext` に依存せず、分離テストから直接呼び出し可能な API を提供する。

```rust
// src/compiler_ws/alloc_runtime.rs

/// アロケータの初期化コード（ヘッダーに埋め込み）
/// global_heap_size: グローバル変数領域のサイズ
pub fn generate_allocator_header(global_heap_size: i64) -> Vec<WhitespaceInstruction> { ... }

/// アロケータのサブルーチン定義
/// __rt_alloc, __rt_free 等のラベル定義と実装
pub fn generate_allocator_subroutines() -> Vec<WhitespaceInstruction> { ... }
```

**設計制約** ([isolated-testing.md](isolated-testing.md) 参照):
- `CodeGenContext` を引数に取らない
- 入出力は `Vec<WhitespaceInstruction>` のみ
- ラベル名は予約プレフィックス `__rt_` を使用
- 分離テスト (`tests/alloc_test.rs`) から直接呼び出し可能

### 2.2 builtin.rs からの呼び出し

builtin.rs の `generate_header` 内で alloc_runtime を呼び出す:

```rust
fn generate_header(...) {
    // ... 既存コード ...
    if ctx.is_alloc_ext() {
        let header = alloc_runtime::generate_allocator_header(global_heap_size);
        instructions.extend(header);
    }
}

fn generate_footer(...) {
    // ... 既存コード ...
    if ctx.is_alloc_ext() {
        let subroutines = alloc_runtime::generate_allocator_subroutines();
        instructions.extend(subroutines);
    }
}
```

内部で以下のサブルーチンを生成:

#### `__runtime_alloc` サブルーチン

**入力**: スタックトップに `requested_size`
**出力**: スタックトップに `ptr` (ユーザーデータ先頭)

**フロー**:
1. `total = max(requested_size + 1, 2)` を計算
2. FSBA サイズクラス選択 (5 段カスケード比較: 2, 4, 8, 16, 32)
3. サイズクラス該当時: FSBA フリーリストからポップ or バンプ拡張
4. 非該当時 (>32 セル): 汎用 First-Fit フリーリスト走査、なければバンプ拡張

Whitespace 命令列 (擬似ニーモニック):

```
__rt_alloc:
    # total = max(requested_size + 1, 2)
    push 1
    add                     # stack: [requested_size + 1]
    dup
    push 2
    sub
    jn _alloc_min_size      # requested_size + 1 < 2 → 最小サイズに補正
    jmp _alloc_search
_alloc_min_size:
    pop
    push 2                  # total = 2
_alloc_search:
    # total はスタックトップに保持
    # prev_next_addr = ALLOC_FREE_HEAD のアドレス (5)
    # curr = heap[ALLOC_FREE_HEAD]
    push 5                  # ALLOC_FREE_HEAD のアドレス
    push 5
    retrieve                # stack: [total, prev_next_addr(5), curr]

_alloc_loop:
    # stack: [total, prev_next_addr, curr]
    dup
    jz _alloc_bump          # curr == 0 → バンプ拡張

    # curr_size = heap[curr]
    dup
    retrieve                # stack: [total, prev_next, curr, curr_size]

    # curr_size >= total ?
    copy 3                  # stack: [total, prev_next, curr, curr_size, total]
    sub                     # stack: [total, prev_next, curr, curr_size - total]
    dup
    jn _alloc_next          # curr_size < total → 次へ

    # Found! curr_size - total がスタックトップ
    # 分割判定: curr_size - total >= 2 ?
    dup
    push 2
    sub
    jn _alloc_use_whole     # 残余 < 2 → 分割せず

    # --- 分割 ---
    # stack: [total, prev_next, curr, remainder_size]
    # remainder = curr + total
    copy 1                  # curr
    copy 4                  # total
    add                     # remainder = curr + total
    # heap[remainder] = remainder_size
    swap                    # [total, prev_next, curr, remainder, remainder_size]
    copy 1                  # [total, prev_next, curr, remainder, remainder_size, remainder]
    swap                    # [total, prev_next, curr, remainder, remainder, remainder_size]
    store                   # heap[remainder] = remainder_size
    # heap[remainder + 1] = heap[curr + 1]  (next pointer)
    dup                     # remainder
    push 1
    add                     # remainder + 1
    copy 2                  # curr
    push 1
    add
    retrieve                # heap[curr + 1]
    store                   # heap[remainder + 1] = heap[curr + 1]
    # heap[prev_next_addr] = remainder
    copy 2                  # prev_next
    swap                    # [total, prev_next, curr, prev_next, remainder]
    store                   # heap[prev_next] = remainder
    # heap[curr] = total
    copy 2                  # total
    copy 1                  # curr
    swap
    store                   # heap[curr] = total
    # return curr + 1
    swap                    # [total, prev_next, curr]
    swap
    pop                     # [total, curr]
    swap
    pop                     # [curr]
    push 1
    add                     # curr + 1
    ret

_alloc_use_whole:
    # 分割なし
    # stack: [total, prev_next, curr, remainder(余り、使わない)]
    pop
    # heap[prev_next] = heap[curr + 1]
    dup                     # curr
    push 1
    add
    retrieve                # heap[curr + 1]
    copy 2                  # prev_next
    swap
    store                   # heap[prev_next] = heap[curr + 1]
    # return curr + 1
    swap
    pop                     # [total, curr]
    swap
    pop                     # [curr]
    push 1
    add
    ret

_alloc_next:
    # curr_size < total → 次のフリーブロックへ
    # stack: [total, prev_next, curr, (curr_size - total)(負)]
    pop
    # prev_next = curr + 1
    dup
    push 1
    add                     # curr + 1
    swap                    # [total, prev_next, curr+1, curr]

    # curr = heap[curr + 1]
    push 1
    add
    retrieve                # heap[curr + 1]
    # stack: [total, prev_next_old, new_prev_next, new_curr]
    # prev_next_old を削除
    copy 2                  # [total, prev_next_old, new_prev_next, new_curr, prev_next_old]...
    # (スタック操作が複雑になるため、実装時に調整要)

    jmp _alloc_loop

_alloc_bump:
    # stack: [total, prev_next, curr(=0)]
    pop
    pop                     # [total]
    # ptr = heap[ALLOC_HEAP_TOP]
    push 6                  # ALLOC_HEAP_TOP
    retrieve                # heap_top
    # heap[heap_top] = total
    dup
    copy 2                  # total
    swap
    store                   # heap[heap_top] = total
    # heap[ALLOC_HEAP_TOP] = heap_top + total
    dup                     # heap_top
    copy 2                  # total
    add                     # heap_top + total
    push 6
    swap
    store                   # heap[ALLOC_HEAP_TOP] = heap_top + total
    # return heap_top + 1
    push 1
    add
    swap
    pop                     # [heap_top + 1]
    ret
```

**注**: 上記は擬似ニーモニック。実際の Whitespace 命令列はコンパイラが `WhitespaceInstruction` enum として生成する。スタック操作の正確な組み立ては実装時に検証・調整が必要。

#### `__runtime_free` サブルーチン

**フロー**:
1. `block = ptr - 1` でヘッダー取得
2. `block_size = heap[block]` を読み取り
3. サイズクラス判定 (block_size == 2/4/8/16/32 ?)
4. FSBA 該当: 該当クラスのフリーリストにプッシュ
5. 非該当: 汎用フリーリストにプッシュ

```
__rt_free:
    # stack: [ptr]
    push 1
    sub                     # block = ptr - 1
    dup
    retrieve                # block_size = heap[block]

    # サイズクラス判定 (カスケード比較)
    dup
    push 2
    sub
    jz _free_class0         # block_size == 2 → class 0

    dup
    push 4
    sub
    jz _free_class1         # block_size == 4 → class 1

    dup
    push 8
    sub
    jz _free_class2         # block_size == 8 → class 2

    dup
    push 16
    sub
    jz _free_class3         # block_size == 16 → class 3

    dup
    push 32
    sub
    jz _free_class4         # block_size == 32 → class 4

    pop                     # block_size を破棄
    # 汎用フリーリストへプッシュ
    dup
    push 1
    add                     # block + 1
    push 5                  # ALLOC_FREE_HEAD
    retrieve                # old_head
    store                   # heap[block + 1] = old_head
    push 5
    swap
    store                   # heap[ALLOC_FREE_HEAD] = block
    ret

_free_class0:
    pop                     # block_size を破棄
    # FSBA class 0 フリーリストへプッシュ
    # free_head_addr = heap[FSBA_TABLE_PTR] + 0
    push 7
    retrieve                # table_ptr
    # (class index 0 なのでオフセット加算不要)
    # heap[block + 1] = heap[free_head_addr]
    # heap[free_head_addr] = block
    # ... (各クラス共通ヘルパーへ)
    jmp _free_fsba_push

# _free_class1 ... _free_class4 も同様パターン
# class index をスタックに積んで _free_fsba_push へジャンプ

_free_fsba_push:
    # stack: [block, free_head_addr]
    # heap[block + 1] = heap[free_head_addr]
    copy 1                  # block
    push 1
    add                     # block + 1
    copy 1                  # free_head_addr
    retrieve                # heap[free_head_addr]
    store                   # heap[block + 1] = heap[free_head_addr]
    # heap[free_head_addr] = block
    swap                    # [free_head_addr, block]
    store                   # heap[free_head_addr] = block
    ret
```

### 2.2 ラベル管理

アロケータサブルーチンのラベルを `LabelAllocator` で管理する。

専用ラベルの命名方針:
- ランタイムラベルは通常のラベル割り当て範囲内で確保
- `ctx.new_label()` で必要数を事前確保し、ブロック内でオフセットでアクセス

## Phase 3: スタックフレーム確保の移行

### 3.1 builtin.rs: `generate_local_allocate` の分岐

```rust
pub fn generate_local_allocate(
    instructions: &mut Vec<WhitespaceInstruction>,
    ctx: &mut CodeGenContext,
    local_heap_size: i64,
) {
    if ctx.is_alloc_ext() {
        generate_local_allocate_via_alloc(instructions, ctx, local_heap_size);
    } else {
        generate_local_allocate_bump(instructions, ctx, local_heap_size);
    }
}
```

#### `generate_local_allocate_via_alloc`

```
# 現在の LOCAL_HEAP_BEGIN をスタックに退避
push 2                      # LOCAL_HEAP_BEGIN address
retrieve                    # old_local_heap_begin

# alloc(local_heap_size) を呼び出し
push {local_heap_size}
call __rt_alloc             # stack: [old_lhb, new_frame_ptr]

# LOCAL_HEAP_BEGIN = new_frame_ptr
push 2                      # LOCAL_HEAP_BEGIN address
swap
store                       # heap[2] = new_frame_ptr

# old_lhb はスタック上に残す (deallocate 時の復元用)
```

#### `generate_local_deallocate_via_alloc`

```
# stack: [old_lhb]
# free(heap[LOCAL_HEAP_BEGIN]) → 現フレームを解放
push 2
retrieve                    # current_frame_ptr
call __rt_free

# LOCAL_HEAP_BEGIN = old_lhb
push 2
swap
store                       # heap[2] = old_lhb
```

### 3.2 statement.rs への影響

`generate_function_definition` の引数配置ロジックは変更不要。現在も `LOCAL_HEAP_END` ベースで引数を配置し、allocate 後に `LOCAL_HEAP_BEGIN` がその領域を指す。

**alloc 方式での引数配置**:

allocate_via_alloc はフレーム確保後に `LOCAL_HEAP_BEGIN` を返り値ポインタとして設定する。引数の配置は以下の手順:

1. 引数値をスタックに積む（呼び出し側）
2. `alloc(frame_size)` でフレーム確保、`LOCAL_HEAP_BEGIN` にセット
3. スタック上の引数値を `heap[LOCAL_HEAP_BEGIN + offset]` に格納

ただし、現在の実装では引数を **allocate 前の `LOCAL_HEAP_END` に書き込み**、その後 allocate で `LOCAL_HEAP_BEGIN` がそのアドレスを指すようにしている。

alloc 方式では **allocate 後に** `LOCAL_HEAP_BEGIN` 経由で引数を書き込む必要がある。この変更は `statement.rs` の `generate_function_definition` に影響する。

具体的な変更:

```rust
// 現在 (alloc 無効時):
// 1. 引数を LOCAL_HEAP_END + offset に書き込み
// 2. generate_local_allocate (LOCAL_HEAP_BEGIN = LOCAL_HEAP_END)

// alloc 有効時:
// 1. generate_local_allocate_via_alloc (alloc + LOCAL_HEAP_BEGIN 更新)
// 2. 引数を LOCAL_HEAP_BEGIN + offset に書き込み
```

## Phase 4: `__alloc`/`__free` 組み込み関数

### 4.1 expression.rs

`generate_function_call` に `__alloc` と `__free` のハンドリングを追加:

```rust
"__alloc" if ctx.is_alloc_ext() => {
    // 引数: size (1個)
    generate_expression(instructions, ctx, &args[0]);
    // __runtime_alloc を呼び出し
    instructions.push(WhitespaceInstruction::Call(ctx.get_runtime_alloc_label()));
    // 戻り値: スタックトップに ptr
}

"__free" if ctx.is_alloc_ext() => {
    // 引数: ptr (1個)
    generate_expression(instructions, ctx, &args[0]);
    // __runtime_free を呼び出し
    instructions.push(WhitespaceInstruction::Call(ctx.get_runtime_free_label()));
    // 戻り値: なし → 0 を push (式としての戻り値)
    instructions.push(WhitespaceInstruction::Push(0));
}
```

### 4.2 semantic_analyzer への影響

`__alloc` と `__free` を新しい組み込み関数として認識させる。

- `__alloc(size)`: 引数 1、戻り値あり（ポインタ）
- `__free(ptr)`: 引数 1、戻り値なし（0）

ただし、`--std-ext alloc` がコンパイル時のみのオプションであるため:
- **インタプリタモード**: `__alloc`/`__free` は将来実装（またはランタイムエラー）
- **コンパイルモード**: `alloc_ext` が true のときのみ有効なコード生成

semantic_analyzer は `--std-ext` を知らないため、関数名が未定義としてエラーにならないよう調整が必要。方針:
- 組み込み関数として常に認識し、実行時/コンパイル時に `alloc_ext` が無効なら適切にエラー

### 4.3 spec.md への追記

`__alloc`/`__free` の仕様を言語仕様に追加:

```markdown
### メモリ管理組み込み関数 (--std-ext alloc)

| 関数 | 説明 |
|------|------|
| `__alloc(n)` | n セル分のメモリを確保し、先頭アドレスを返す |
| `__free(ptr)` | `__alloc` で確保したメモリを解放する |
```

## 変更ファイル一覧

| ファイル | Phase | 変更内容 |
|---|---|---|
| `src/compile_property.rs` | 1 | `TargetExtension::Alloc` 追加 |
| `src/bin/nospace20.rs` | 1 | `CliTargetExt::Alloc` 追加 |
| `src/compiler_ws/memory.rs` | 1 | 定数追加 (`ALLOC_FREE_HEAD`, `ALLOC_HEAP_TOP`, `FSBA_TABLE_PTR`) |
| `src/compiler_ws/context.rs` | 1 | `alloc_ext` フラグ、`CodeGenOptions` 構造体 |
| `src/compiler_ws/mod.rs` | 1 | `compile_with_options` 引数拡張、`alloc_runtime` 公開 |
| `src/lib.rs` | 1 | 公開 API 更新 |
| `src/compiler_ws/alloc_runtime.rs` | 2 | **新規**: アロケータランタイム生成（FSBA + 汎用）。分離テスト対応 |
| `src/compiler_ws/builtin.rs` | 2, 3 | `alloc_runtime` 呼び出し、`generate_local_allocate/deallocate` の分岐 |
| `src/compiler_ws/statement.rs` | 3 | 関数定義の引数配置ロジック変更 |
| `src/compiler_ws/expression.rs` | 4 | `__alloc`/`__free` のコード生成 |
| `src/semantic_analyzer/mod.rs` | 4 | `__alloc`/`__free` 組み込み関数認識（検討要） |
| `spec.md` | 4 | `__alloc`/`__free` 仕様追記 |
| `src/wasm_api.rs` | 1 | alloc_ext パラメータ対応（必要に応じて） |
| `build.rs` | 2 | `generate_alloc_tests()` 追加 |
| `tests/alloc_test.rs` | 2 | **新規**: 分離テストランナー + ミニコンパイラ |
| `resources/tests_alloc/` | 2 | **新規**: JSON テスト仕様ファイル群 |
