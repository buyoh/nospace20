# AllocRuntime trait 設計

## 概要

メモリアロケータの Whitespace コード生成を trait として抽象化する。
これにより、バンプアロケータ（現行方式）と FSBA+First-Fit アロケータを同一インターフェースで差し替え可能にする。

## 設計原則（重要）

**全ての AllocRuntime 実装は `__rt_alloc(size) → ptr` / `__rt_free(ptr)` サブルーチンを提供しなければならない。**

スタックフレーム確保は内部的にアロケータ（`__rt_alloc`/`__rt_free`）を使用する。
これは BumpAllocRuntime も例外ではない。バンプ方式でもサブルーチンとして
`__rt_alloc`/`__rt_free` を生成し、プロローグ/エピローグはこれらを呼び出す。

この原則により:
1. **テスト統一性**: JSON 分離テストフレームワークが全実装をテスト可能
2. **抽象化の一貫性**: alloc/free が全アロケータの基本プリミティブ
3. **Phase 構成の簡素化**: スタックフレーム移行フェーズが不要

## trait 定義

```rust
/// ランタイムメモリアロケータの WS コード生成を担当する trait。
///
/// 各メソッドは Whitespace 命令列を返す。実装を差し替えることで、
/// 異なるメモリ管理方式を選択可能にする。
pub trait AllocRuntime {
    /// ヘッダー部分のメモリ初期化コードを生成。
    ///
    /// ヒープの予約アドレスを初期化する。
    /// global_heap_size: グローバル変数 + static 変数の合計サイズ
    fn generate_memory_init(&self, global_heap_size: i64) -> WsProgram;

    /// フッター部分のサブルーチン定義コードを生成。
    ///
    /// アロケータが使用するサブルーチン（__rt_alloc, __rt_free 等）を定義する。
    /// **全ての実装は最低限 `__rt_alloc` と `__rt_free` を定義しなければならない。**
    fn generate_subroutines(&self) -> WsProgram;

    /// 関数プロローグ: 引数コピー + フレーム確保
    ///
    /// 内部で `__rt_alloc` を呼び出してフレームを確保する。
    ///
    /// スタック入力: `[..., arg(n-1), ..., arg(0)]`
    /// スタック出力: `[..., old_context]`
    fn generate_function_prologue(
        &self,
        local_heap_size: i64,
        arg_offsets: &[i64],
    ) -> WsProgram;

    /// 関数エピローグ: フレーム解放 + コンテキスト復元
    ///
    /// 内部で `__rt_free` を呼び出してフレームを解放する。
    ///
    /// スタック入力: `[..., old_context]`
    /// スタック出力: `[...]`
    fn generate_function_epilogue(&self) -> WsProgram;
}
```

## `__rt_alloc` / `__rt_free` サブルーチン規約

全ての AllocRuntime 実装が提供するサブルーチンの規約:

### `__rt_alloc(size) → ptr`

- **スタック入力**: `[..., size]`
- **スタック出力**: `[..., ptr]`
- `size` セル分の連続領域を確保し、先頭アドレス `ptr` を返す
- `ptr` から `ptr + size - 1` までの領域は呼び出し元が自由に使用可能

### `__rt_free(ptr)`

- **スタック入力**: `[..., ptr]`
- **スタック出力**: `[...]`
- `ptr` で確保された領域を解放する
- 実装によっては即座に再利用可能（FSBA）、または LIFO 順のみ有効（バンプ方式）

## BumpAllocRuntime

### サブルーチン

BumpAllocRuntime は `__rt_alloc` / `__rt_free` を以下のように実装する:

#### `__rt_alloc(size) → ptr`

```
ptr = heap[LOCAL_HEAP_END]
heap[LOCAL_HEAP_END] = ptr + size
return ptr
```

WS 疑似コード:
```
label __rt_alloc:
  ; スタック: [size]
  push LOCAL_HEAP_END
  retrieve              ; [size, LHE]
  swap                  ; [LHE, size]
  push LOCAL_HEAP_END   ; [LHE, size, &LHE]
  copy(2)               ; [LHE, size, &LHE, LHE]
  copy(2)               ; [LHE, size, &LHE, LHE, size]
  add                   ; [LHE, size, &LHE, LHE+size]
  store                 ; [LHE, size]  heap[LHE] = LHE+size
  drop                  ; [LHE]  ← ptr
  return
```

#### `__rt_free(ptr)`

```
heap[LOCAL_HEAP_END] = ptr
```

WS 疑似コード:
```
label __rt_free:
  ; スタック: [ptr]
  push LOCAL_HEAP_END   ; [ptr, &LHE]
  swap                  ; [&LHE, ptr]
  store                 ; heap[LHE] = ptr
  return
```

> 注: BumpAllocRuntime の `__rt_free(ptr)` は LIFO 順序でのみ正しく動作する。
> 一般的な alloc/free パターン（任意順の解放）では FSBA を使用する。

### プロローグ動作

1. `old_LHB` をスタックに退避
2. `__rt_alloc(local_heap_size)` を呼び出し → `ptr`
3. `LOCAL_HEAP_BEGIN = ptr`
4. 引数を `heap[ptr + offset]` にコピー
5. `old_LHB` がスタックに残る（old_context）

> 注: 実際の WS 命令列はスタック操作の最適化により上記とは異なる場合がある。
> 引数コピーと alloc 呼び出しの順序は、スタックレイアウトに応じて調整する。

### エピローグ動作

1. `ptr = LOCAL_HEAP_BEGIN`
2. `LOCAL_HEAP_BEGIN = pop(old_LHB)` （スタックから復元）
3. `__rt_free(ptr)`

### メモリ初期化

```
heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR
heap[LOCAL_HEAP_END] = GLOBAL_PTR + global_heap_size
```

## FsbaFirstFitAllocRuntime（将来実装）

### サブルーチン

`__rt_alloc`, `__rt_free` を FSBA + First-Fit アルゴリズムで実装。
詳細は [fixed-size-block-allocator.md](fixed-size-block-allocator.md) を参照。

### プロローグ動作

BumpAllocRuntime と同じ高レベルフロー:

1. `old_LHB` をスタックに退避
2. `__rt_alloc(local_heap_size)` を呼び出し → `ptr`
3. `LOCAL_HEAP_BEGIN = ptr`
4. 引数を `heap[ptr + offset]` にコピー
5. `old_LHB` がスタックに残る（old_context）

> 注: プロローグ/エピローグのフローは全実装で共通化できる可能性があるが、
> スタック操作の最適化が実装依存なため、trait メソッドとして各実装が個別に生成する。

### エピローグ動作

BumpAllocRuntime と同じ高レベルフロー:

1. `ptr = LOCAL_HEAP_BEGIN`
2. `LOCAL_HEAP_BEGIN = pop(old_LHB)`
3. `__rt_free(ptr)`

### メモリ初期化

```
heap[LOCAL_HEAP_BEGIN] = 0
heap[ALLOC_FREE_HEAD] = 0
heap[ALLOC_HEAP_TOP] = GLOBAL_PTR + global_heap_size + FSBA_CLASS_COUNT
heap[FSBA_TABLE_PTR] = GLOBAL_PTR + global_heap_size
for i in 0..FSBA_CLASS_COUNT:
    heap[FSBA_TABLE_PTR + i] = 0
```

## 呼び出しフロー

### statement.rs（generate_function_definition）

```rust
// OLD (Phase 1 以前):
// 1. 引数コピー (LHE ベース)
// 2. builtin::generate_local_allocate()
// 3. 関数本体
// 4. builtin::generate_local_deallocate()

// NEW (Phase 2):
// 1. ctx.alloc_runtime().generate_function_prologue(size, &arg_offsets)
//    - 引数コピー + __rt_alloc + フレーム確保を一括処理
// 2. 関数本体
// 3. ctx.alloc_runtime().generate_function_epilogue()
//    - __rt_free + コンテキスト復元を一括処理
```

### builtin.rs（generate_header / generate_footer）

```rust
// generate_header:
//   alloc_runtime.generate_memory_init(global_heap_size)

// generate_footer:
//   alloc_runtime.generate_subroutines()
//   → BumpAllocRuntime: __rt_alloc, __rt_free の定義
//   → FsbaFirstFitAllocRuntime: __rt_alloc, __rt_free + FSBA 内部サブルーチン
```

## CodeGenContext への統合

```rust
pub struct CodeGenContext<'a> {
    // ... existing fields ...
    alloc_runtime: &'a dyn AllocRuntime,
}
```

### コンストラクタ変更

```rust
pub fn new_with_options(
    scope: &'a Scope,
    debug_ext: bool,
    alloc_runtime: &'a dyn AllocRuntime,
) -> Self
```

### enter_function での伝播

`alloc_runtime` 参照は子コンテキストにコピーされる。

## 現行実装との差分

### 修正が必要な箇所

現在の `BumpAllocRuntime` 実装（Phase 2 完了時点）は以下の点で本設計と乖離している:

1. **`generate_subroutines()` が空**: `__rt_alloc`/`__rt_free` サブルーチンを生成していない
2. **プロローグがインライン割り当て**: `__rt_alloc` を呼ばず直接 LHE を操作
3. **エピローグがインライン解放**: `__rt_free` を呼ばず直接 LHE を操作

#### 修正方針

Phase 2 の BumpAllocRuntime を以下のように修正する（Phase 2 修正として扱う）:

1. `generate_subroutines()` に `__rt_alloc`/`__rt_free` のサブルーチン定義を追加
2. `generate_function_prologue()` を `__rt_alloc` 呼び出し方式に変更
3. `generate_function_epilogue()` を `__rt_free` 呼び出し方式に変更
4. 既存テスト全パスを確認（動作は等価）

## 設計原則

1. **全実装が `__rt_alloc`/`__rt_free` を提供**: alloc/free はアロケータの基本プリミティブ
2. **プロローグ/エピローグは alloc/free を使用**: スタックフレームも allocator 経由で確保
3. **trait は Whitespace コード生成に特化**: ヒープアドレス等の具体値は実装が決定
4. **CodeGenContext は trait に非依存**: `&dyn AllocRuntime` で間接参照
5. **テスト容易性**: JSON 分離テストが全実装に適用可能
