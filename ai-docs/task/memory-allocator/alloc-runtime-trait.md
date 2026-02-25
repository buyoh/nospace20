# AllocRuntime trait 設計

## 概要

メモリアロケータの Whitespace コード生成を trait として抽象化する。
これにより、バンプアロケータ（現行方式）と FSBA+First-Fit アロケータを同一インターフェースで差し替え可能にする。

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
    /// サブルーチンが不要な実装（バンプ方式等）は空の WsProgram を返す。
    fn generate_subroutines(&self) -> WsProgram;

    /// 関数プロローグ: 引数コピー + フレーム確保
    ///
    /// スタック入力: `[..., arg(n-1), ..., arg(0)]`
    /// スタック出力: `[..., old_context]`
    ///
    /// 呼び出し後:
    /// - `heap[LOCAL_HEAP_BEGIN]` = 新フレーム先頭アドレス
    /// - 引数は `heap[LOCAL_HEAP_BEGIN + arg_offsets[i]]` に格納済み
    /// - `old_context` はエピローグで使用するコンテキスト復元データ
    fn generate_function_prologue(
        &self,
        local_heap_size: i64,
        arg_offsets: &[i64],
    ) -> WsProgram;

    /// 関数エピローグ: フレーム解放 + コンテキスト復元
    ///
    /// スタック入力: `[..., old_context]`
    /// スタック出力: `[...]`
    fn generate_function_epilogue(&self) -> WsProgram;
}
```

## BumpAllocRuntime（現行方式）

現在の `builtin.rs` の `generate_local_allocate` / `generate_local_deallocate` および
`statement.rs` の引数コピーロジックをそのまま trait 実装に移行する。

### プロローグ動作

1. 引数を `heap[LOCAL_HEAP_END + offset]` にコピー（LHE は新フレームの先頭）
2. `old_LHB` をスタックに退避
3. `LOCAL_HEAP_BEGIN = LOCAL_HEAP_END`
4. `LOCAL_HEAP_END += local_heap_size`

### エピローグ動作

1. `LOCAL_HEAP_END = LOCAL_HEAP_BEGIN`
2. `LOCAL_HEAP_BEGIN = pop(old_LHB)`

### メモリ初期化

```
heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR
heap[LOCAL_HEAP_END] = GLOBAL_PTR + global_heap_size
```

### サブルーチン

なし（空の WsProgram）

## FsbaFirstFitAllocRuntime（将来実装）

### プロローグ動作

1. `old_LHB` を一時ヒープアドレス（addr 0）に退避
2. `__rt_alloc(local_heap_size)` を呼び出し → `ptr`
3. `LOCAL_HEAP_BEGIN = ptr`
4. 引数を `heap[LOCAL_HEAP_BEGIN + offset]` にコピー
5. `heap[0]`（退避した old_LHB）をスタックにプッシュ

### エピローグ動作

1. `ptr = heap[LOCAL_HEAP_BEGIN]`
2. `LOCAL_HEAP_BEGIN = pop(old_LHB)`（スタックから復元）
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

### サブルーチン

`__rt_alloc`, `__rt_free` のサブルーチン定義。

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
//    - 引数コピー + フレーム確保を一括処理
// 2. 関数本体
// 3. ctx.alloc_runtime().generate_function_epilogue()
```

### builtin.rs（generate_header / generate_footer）

```rust
// generate_header:
//   alloc_runtime.generate_memory_init(global_heap_size)

// generate_footer:
//   alloc_runtime.generate_subroutines()
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

## 設計原則

1. **BumpAllocRuntime は現行動作と完全に同一**: リファクタリングで動作変更なし
2. **trait は Whitespace コード生成に特化**: ヒープアドレス等の具体値は実装が決定
3. **CodeGenContext は trait に非依存**: `&dyn AllocRuntime` で間接参照
4. **テスト容易性**: trait 実装は単体で WS 命令列を生成可能（分離テストからも利用可能）
