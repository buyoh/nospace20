# インタプリタ メモリアロケータ実装

## 概要

インタプリタにメモリアロケータを導入し、`__alloc`/`__free` をインタプリタモードでも利用可能にする。
同時に、グローバル変数・ローカルスタック・static 変数もアロケータ経由で管理するよう変更する。

現在のインタプリタでは `__alloc`/`__free` は `panic!` でエラーとなる（Whitespace コンパイラ専用）。
本タスクではインタプリタ独自のメモリアロケータを実装し、統一的なメモリモデルを提供する。

## 背景

### 現状の課題

1. **`__alloc`/`__free` がインタプリタで使えない**: Whitespace コンパイラでのみ対応
2. **メモリアクセスの安全性が低い**: `get_by_address`/`set_by_address` は境界外アクセスで単に panic するが、解放済み領域の検出はできない
3. **アドレス空間の一貫性がない**: グローバル変数・ローカルスコープが別々の `Vec<i64>` で管理され、アドレス解決が線形走査

### 目標

| 目標 | 内容 |
|------|------|
| `__alloc`/`__free` 対応 | インタプリタモードで動的メモリ確保・解放が可能 |
| 統一メモリモデル | グローバル変数・ローカルスタック・static 変数を同一アロケータで管理 |
| メモリ安全性向上 | 解放済み・未割当アドレスへのアクセスを実行時エラーとして検出 |

## 設計方針

### 仮想1次元アドレス空間

- アドレスは1次元的に管理する（0 から単調増加）
- 実際のメモリは `Vec<i64>` の個別ブロックとして確保
- アロケータがアドレス区間（開始アドレス + サイズ）とブロックのマッピングを管理

### メモリブロック管理

- `BTreeMap<i64, MemoryBlock>` で開始アドレスからブロックへのマッピングを保持
- 各ブロックは `data: Vec<i64>`, `is_freed: bool` を持つ
- アドレス検索は `BTreeMap::range(..=addr)` で O(log n)

### エラーハンドリング

- 未割当アドレスへのアクセス → `panic!("runtime error: ...")`
- 解放済みアドレスへのアクセス → `panic!("runtime error: ...")`
- 既存の実行時エラー（ゼロ除算、アサーション失敗等）と統一的に `panic!` で処理
- テストでは `std::panic::catch_unwind` でキャッチ（既存の `runtime_error` テスト基盤を利用）

## ドキュメント

| ドキュメント | 内容 |
|---|---|
| [allocator-design.md](allocator-design.md) | InterpreterAllocator のデータ構造とアルゴリズム |
| [migration-design.md](migration-design.md) | 既存メモリ管理からアロケータへの移行設計 |
| [testing-plan.md](testing-plan.md) | テスト計画 |

## Phase 一覧

| Phase | 内容 | 依存 | 規模 |
|---|---|---|---|
| Phase 1 | `InterpreterAllocator` 実装 + ユニットテスト | なし | 小 |
| Phase 2 | `Environment` への統合（グローバル変数・static 変数） | Phase 1 | 中 |
| Phase 3 | `LocalEnvironment` のスコープ管理を移行 | Phase 2 | 中 |
| Phase 4 | `__alloc`/`__free` 組み込み関数の実装 | Phase 3 | 小 |
| Phase 5 | テスト更新・追加 | Phase 4 | 小 |

### Phase 詳細

#### Phase 1: InterpreterAllocator 実装

新規モジュール `src/interpreter/allocator.rs` を作成。

- `InterpreterAllocator` struct
  - `blocks: BTreeMap<i64, MemoryBlock>` — アドレス→ブロックのマッピング
  - `next_addr: i64` — 次の割当アドレス（バンプポインタ）
- `alloc(size: usize) -> i64` — 新しいブロックを確保し、開始アドレスを返す
- `free(addr: i64)` — ブロックを解放済みにマーク（存在しないアドレスの場合は panic）
- `get(addr: i64) -> i64` — アドレスから値を読み取り
- `set(addr: i64, value: i64)` — アドレスに値を書き込み
- ユニットテスト（alloc/free/get/set、エラーケース）

#### Phase 2: Environment への統合

`Environment` に `InterpreterAllocator` を追加し、既存のメモリ管理を移行。

- `Environment` に `allocator: InterpreterAllocator` フィールドを追加
- `global_variables: Vec<i64>` → アロケータ経由に変更
  - `global_base_addr: i64` フィールドを追加
  - グローバル変数の初期化時に `allocator.alloc(variable_count)` を呼び出し
- `function_static_storage: BTreeMap<usize, Vec<i64>>` → アロケータ経由に変更
  - `function_static_addrs: BTreeMap<usize, i64>` でベースアドレスのみを保持
  - static ストレージの読み書きはアロケータの `get`/`set` を使用

#### Phase 3: LocalEnvironment のスコープ管理移行

ローカル変数のスコープ管理をアロケータに移行。

- `scope_stack: Vec<Vec<i64>>` → `scope_stack: Vec<i64>` に変更
  - 各エントリはアロケータ上のベースアドレス
- `enter_block(scope)` → `allocator.alloc(scope.variable_count)` してベースアドレスを push
- `leave_block()` → ベースアドレスを pop し `allocator.free(addr)`
- `get_variable(id)` → `allocator.get(base_addr + local_index)`
- `set_variable(id, value)` → `allocator.set(base_addr + local_index, value)`
- `resolve_address(id)` → `base_addr + local_index`（アロケータアドレスをそのまま返す）
- `get_by_address(addr)` → `allocator.get(addr)`
- `set_by_address(addr, value)` → `allocator.set(addr, value)`
- `create_uninit_vec` に代わり `allocator.alloc` を使用（ `randomize_uninit` モードの対応が必要）
- 既存テスト全パス確認

#### Phase 4: `__alloc`/`__free` 組み込み関数の実装

- `BuiltinFunctionKind::Alloc` の panic を `allocator.alloc(size)` 呼び出しに変更
- `BuiltinFunctionKind::Free` の panic を `allocator.free(addr)` 呼び出しに変更
- `__free` は 0 を返す（仕様通り）

#### Phase 5: テスト更新・追加

- テストマニフェスト (`resources/tests/test-manifest.yaml`) の alloc 関連テストから `exclude_targets: [interpreter, ...]` の `interpreter` を削除
- 新規テストケース:
  - 解放済みアドレスへのアクセス → runtime_error
  - 未割当アドレスへのアクセス → runtime_error
  - `__free` で無効なアドレスを指定 → runtime_error

## 設計原則

1. **既存テストの維持**: 全ての既存テストがパスすること
2. **panic ベースのエラー**: 既存の runtime_error テスト基盤と統一
3. **段階的な移行**: Phase ごとにテストが通る状態を維持
4. **パフォーマンスは二の次**: インタプリタの主目的はテスト・検証であり、WS コンパイラほどの性能は不要
