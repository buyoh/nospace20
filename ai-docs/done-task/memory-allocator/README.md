# メモリアロケータ実装

## 概要

Whitespace コンパイラにランタイムメモリアロケータを導入する。現在のスタックフレーム管理（`LOCAL_HEAP_BEGIN`/`LOCAL_HEAP_END` によるバンプ方式）をアロケータベースに移行し、最終的にはユーザーコードからの動的ヒープ確保（`__alloc`/`__free`）を可能にする。

この機能は Whitespace コンパイル時にとって非常に複雑であるため、`--std-ext alloc` による明示的な有効化時のみ利用可能とする。

## 背景

### 現状の課題

1. **動的メモリ確保が不可能**: 配列サイズはコンパイル時定数のみ。実行時にサイズが決まるバッファを確保する手段がない
2. **セルフコンパイラの制約**: AST ノード等を固定長配列＋手動インデックスで管理するしかない
3. **ローカルフレームが一方通行**: `LOCAL_HEAP_END` は関数呼び出しで伸長するのみで、フレーム間の隙間を再利用しない
4. **メモリ管理の抽象化不足**: スタックフレームとヒープ領域が別々の仕組みで管理されている

### 目標

| 段階 | 内容 |
|------|------|
| 1. アロケータ基盤 | Whitespace サブルーチンとしてメモリアロケータを実装 |
| 2. スタックフレーム移行 | 関数呼び出し時のフレーム確保をアロケータ経由に変更 |
| 3. ヒープ API 公開 | `__alloc(size)` / `__free(ptr)` を nospace 組み込み関数として提供 |

## ドキュメント

| ドキュメント | 内容 |
|---|---|
| [allocator-design.md](allocator-design.md) | コアアロケータのアルゴリズムとデータ構造 |
| [fixed-size-block-allocator.md](fixed-size-block-allocator.md) | 固定サイズブロックアロケータ (FSBA) の詳細設計 |
| [heap-layout.md](heap-layout.md) | 新しいヒープメモリレイアウト設計 |
| [compiler-changes.md](compiler-changes.md) | compiler_ws モジュールへの変更設計 |
| [std-ext-integration.md](std-ext-integration.md) | `--std-ext alloc` の統合と条件分岐 |
| [testing-strategy.md](testing-strategy.md) | テスト計画（4 層テスト構造） |
| [isolated-testing.md](isolated-testing.md) | 分離テスト設計（JSON ミニ言語・ミニコンパイラ） |
| [alloc-runtime-trait.md](alloc-runtime-trait.md) | AllocRuntime trait 設計 |

## Phase 一覧

| Phase | 内容 | 依存 | 規模 |
|---|---|---|---|
| Phase 1 | `--std-ext alloc` の追加と基盤整備 | なし | 小 |
| Phase 2 | AllocRuntime trait 抽象化 + BumpAllocRuntime 実装（`__rt_alloc`/`__rt_free` サブルーチン含む） | Phase 1 | 中 |
| Phase 3 | 分離テストフレームワーク設計・実装 | Phase 2 | 中 |
| Phase 4 | FSBA + First-Fit アロケータ実装 + 分離テスト (L1/L2) | Phase 3 | 大 |
| Phase 5 | `__alloc`/`__free` 組み込み関数の公開 + E2E テスト (L3) | Phase 4 | 中 |

### Phase 詳細

#### Phase 1: `--std-ext alloc` の追加と基盤整備

- `TargetExtension::Alloc` の追加（compile_property.rs）
- CLI に `--std-ext alloc` オプション追加（nospace20.rs）
- バリデーション（`--std=ws --mode=compile` 時のみ有効）
- memory.rs に新しい定数追加（ALLOC_FREE_HEAD, ALLOC_HEAP_TOP, FSBA_TABLE_PTR）
- context.rs に `alloc_ext` フラグ追加
- compile_with_options に `alloc_ext` パラメータ追加

#### Phase 2: AllocRuntime trait 抽象化 + BumpAllocRuntime 実装

現在の「stack のみ＋バンプ方式」を trait インターフェースとして再設計する。
**全実装が `__rt_alloc`/`__rt_free` サブルーチンを提供する設計**とする。

- `AllocRuntime` trait 定義（alloc_runtime.rs 新規モジュール）
  - `generate_memory_init`: ヘッダーのメモリ初期化
  - `generate_subroutines`: サブルーチン定義（`__rt_alloc`, `__rt_free` 必須）
  - `generate_function_prologue`: `__rt_alloc` を呼び出してフレーム確保 + 引数コピー
  - `generate_function_epilogue`: `__rt_free` を呼び出してフレーム解放 + コンテキスト復元
- `BumpAllocRuntime`: バンプ方式の `__rt_alloc`/`__rt_free` サブルーチン + プロローグ/エピローグ
- `CodeGenContext` に `&dyn AllocRuntime` を追加
- `builtin.rs` / `statement.rs` を trait 経由に変更
- **既存テスト全パス確認**（動作変更なしのリファクタリング）

詳細は [alloc-runtime-trait.md](alloc-runtime-trait.md) を参照。

#### Phase 3: 分離テストフレームワーク設計・実装

アロケータ単体をテスト可能な JSON ミニ言語 + ミニコンパイラを実装する。

- JSON テスト仕様フォーマット（[isolated-testing.md](isolated-testing.md) 参照）
- テスト用ミニコンパイラ（tests/alloc_test.rs）
- build.rs テスト生成
- BumpAllocRuntime の基本テスト（`__rt_alloc`/`__rt_free` サブルーチンを直接テスト）

#### Phase 4: FSBA + First-Fit アロケータ実装

- `FsbaFirstFitAllocRuntime`: AllocRuntime trait の新しい実装
- alloc_runtime.rs 内に WS サブルーチン生成コード
- 分離テスト (L1/L2) 全件 + 統合/回帰テスト (L3/L4)
- `--std-ext alloc` 時に FsbaFirstFitAllocRuntime を選択

#### Phase 5: `__alloc`/`__free` 公開 + E2E テスト

nospace 組み込み関数として `__alloc(size)` / `__free(ptr)` を提供。

## 設計原則

1. **`--std-ext alloc` 未指定時は既存動作を維持**: 後方互換性を保証
2. **Whitespace 命令セットのみで実装**: アロケータ自体が Whitespace サブルーチンとして動作
3. **全実装が `__rt_alloc`/`__rt_free` を提供**: alloc/free はアロケータの基本プリミティブ
4. **スタックフレームもアロケータ経由**: プロローグ/エピローグは内部的に `__rt_alloc`/`__rt_free` を呼び出す
5. **段階的な移行**: まずアロケータ基盤を作り、次に FSBA 実装、最後にユーザー API
6. **インタプリタは影響なし**: メモリアロケータは Whitespace コンパイル時のみの機能

## 現状

- 2026-02-24: 設計ドキュメント作成
- 2026-02-24: 固定サイズブロックアロケータ (FSBA) 設計追加。二層アーキテクチャ (FSBA + 汎用 First-Fit) に変更
- 2026-02-24: 分離テスト設計追加。JSON ミニ言語によるアロケータ単体テスト。テスト 4 層構造 (L1-L4) に再構成。Phase 5 を廃止し、テストを各 Phase に統合
- 2026-02-25: Phase 細分化。AllocRuntime trait 抽象化フェーズ (Phase 2) を新設。分離テストフレームワークを独立フェーズ (Phase 3) に。旧 Phase 2-4 を Phase 4-6 に繰り下げ
- 2026-02-25: Phase 1 実装完了（`--std-ext alloc` フラグ追加）
- 2026-02-25: Phase 2 実装完了（AllocRuntime trait + BumpAllocRuntime、既存テスト全パス）
- 2026-02-26: 設計修正 — BumpAllocRuntime にも `__rt_alloc`/`__rt_free` サブルーチンを必須化。旧 Phase 5（スタックフレーム移行）を廃止し Phase 構成を 6→5 に縮小。Phase 2 の BumpAllocRuntime 実装を修正予定
- 2026-02-26: Phase 2 修正実装完了 — BumpAllocRuntime が `__rt_alloc`/`__rt_free` サブルーチンを生成するよう変更。プロローグ/エピローグがサブルーチン呼び出しを使用。予約ラベル RT_ALLOC(12)/RT_FREE(13) を label.rs に追加。VM 統合テスト追加。既存テスト全パス確認
- 2026-02-25: Phase 3 実装完了 — 分離テストフレームワーク。JSON テスト仕様 + ミニコンパイラ（tests/alloc_test.rs）+ build.rs テスト自動生成。BumpAllocRuntime 基本テスト 4 件（alloc_basic_001/002, alloc_multi_001, alloc_free_reuse_001）全パス。compiler_ws モジュールを pub 化（テストからのアクセス用）
- 2026-02-27: Phase 4 実装完了 — FsbaFirstFitAllocRuntime 実装。二層アーキテクチャ（FSBA + 汎用 First-Fit + バンプフォールバック）。FSBA 内部ラベル（16-37）を予約しLABEL_OFFSET を 48 に変更。`--std-ext alloc` 時に FsbaFirstFitAllocRuntime を自動選択。L1 分離テスト 11 件（fsba_basic_001/002, fsba_class_reuse_001, fsba_different_class_001, fsba_roundup_001, fsba_large_fallback_001, fsba_alloc_zero_001, fsba_repeated_001, fsba_multi_allocs_001, fsba_split_001, fsba_free_reuse_002）+ L2 unit テスト 6 件追加。全 892 テストパス
- 2026-02-27: Phase 5 実装完了 — `__alloc(size)`/`__free(ptr)` 組み込み関数を nospace 言語に追加。semantic_analyzer で Alloc/Free 認識、compiler_ws で Call(RT_ALLOC)/Call(RT_FREE) へのコンパイル、`--std-ext alloc` 未指定時のコンパイルエラー、インタプリタでのランタイムエラー。E2E テスト (L3) 6 件追加（alloc_basic_001, alloc_array_001, alloc_free_reuse_001, alloc_linked_list_001 + alloc_not_enabled_001, free_not_enabled_001）。docs/spec.md にドキュメント追加。全 916 テストパス
