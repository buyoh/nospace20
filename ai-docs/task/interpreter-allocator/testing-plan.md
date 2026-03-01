# テスト計画

## 既存テストの維持

全 Phase において既存テストが全てパスすることを確認する。
特に以下のカテゴリのテストに注意:

- **参照・間接参照テスト**: `&x` / `*ptr` を使用するテスト（アドレスモデル変更の影響）
- **配列テスト**: 配列インデックスアクセス（ブロック内オフセットの正当性）
- **static 変数テスト**: 関数呼び出し間の永続化が正しく動作すること
- **グローバル変数テスト**: グローバルスコープの変数初期化と参照
- **ユニットテスト** (`src/interpreter/exec.rs` 内): `test_resolve_address_local_variables`, `test_get_set_by_address`, `test_ref_and_deref_integration` 等

## Phase 1: InterpreterAllocator ユニットテスト

`src/interpreter/allocator.rs` 内に `#[cfg(test)]` モジュールとして配置。

| テストケース | 内容 |
|---|---|
| `test_alloc_basic` | `alloc(3)` でアドレスが返り、get/set が動作 |
| `test_alloc_multiple` | 連続して alloc すると異なるアドレスが返る |
| `test_alloc_zero_size` | `alloc(0)` は `alloc(1)` と同等 |
| `test_free_basic` | `free` 後にアクセスすると panic |
| `test_free_invalid_address` | 存在しないアドレスの free で panic |
| `test_double_free` | 同じアドレスの二重 free で panic |
| `test_access_unallocated` | 未割当アドレスへのアクセスで panic |
| `test_access_freed` | 解放済みアドレスへのアクセスで panic |
| `test_block_boundary` | ブロック境界外へのアクセスで panic |
| `test_alloc_uninit_zero` | `alloc_uninit(size, false)` で 0 初期化 |
| `test_alloc_uninit_random` | `alloc_uninit(size, true)` で非 0 値 |

## Phase 5: テストマニフェスト更新

### 既存 alloc テストのインタプリタ有効化

以下のテストの `exclude_targets` から `interpreter` を削除:

| テスト | 変更 |
|---|---|
| `test_builtin_alloc_basic_001` | `exclude_targets: [whitespace]` に変更 |
| `test_builtin_alloc_array_001` | `exclude_targets: [whitespace]` に変更 |
| `test_builtin_alloc_free_reuse_001` | `exclude_targets: [whitespace]` に変更 |
| `test_builtin_alloc_linked_list_001` | `exclude_targets: [whitespace]` に変更 |

**注**: `whitespace` ターゲット（外部 wsc 使用）は引き続き除外。`whitespace-self` は既に有効。

### 新規 runtime_error テストケース

#### `alloc_access_freed_001`

解放済みメモリへのアクセスが runtime error になることを確認。

```nospace
# 解放済みメモリアクセス #
func: __main() {
    let: ptr(__alloc(1));
    *ptr = 42;
    __free(ptr);
    __puti(*ptr);
    return: 0;
}
```

check.json:
```json
{
  "type": "runtime_error",
  "contains": ["freed memory"]
}
```

#### `alloc_invalid_address_001`

未割当アドレスへのアクセスが runtime error になることを確認。

```nospace
# 未割当アドレスアクセス #
func: __main() {
    let: val(*99999);
    return: val;
}
```

check.json:
```json
{
  "type": "runtime_error",
  "contains": ["invalid memory access"]
}
```

#### `alloc_double_free_001`

二重 free が runtime error になることを確認。

```nospace
# 二重 free #
func: __main() {
    let: ptr(__alloc(1));
    __free(ptr);
    __free(ptr);
    return: 0;
}
```

check.json:
```json
{
  "type": "runtime_error",
  "contains": ["double free"]
}
```

#### `alloc_free_invalid_001`

無効アドレスの free が runtime error になることを確認。

```nospace
# 無効アドレスの free #
func: __main() {
    __free(99999);
    return: 0;
}
```

check.json:
```json
{
  "type": "runtime_error",
  "contains": ["free"]
}
```

### テスト対象注意事項

- runtime_error テストは `exclude_targets: [whitespace]` を設定
  - WS コンパイラでは異なるエラーメッセージ/動作になる可能性があるため
  - whitespace-self も判断が必要（WS VM の動作は undefined）
- `alloc_invalid_address_001` はアロケータ導入前のインタプリタでも場合によっては panic する（既存の `get_by_address` の panic）。アロケータ導入後はエラーメッセージが変わる
