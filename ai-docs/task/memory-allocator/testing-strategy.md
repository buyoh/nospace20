# テスト計画

## テスト方針

メモリアロケータのテストは以下の 3 層で実施する:

1. **Unit テスト**: アロケータサブルーチン単体の動作検証（Whitespace VM 上で実行）
2. **統合テスト**: nospace コードからのエンドツーエンドテスト（`resources/tests/` に追加）
3. **既存テストの回帰確認**: `--std-ext alloc` 有効時に既存テストが引き続きパスすること

## Phase 2 テスト: アロケータサブルーチン単体

### Whitespace VM 上での直接テスト

`resources/tests_ws/` に Whitespace アセンブリテストを追加:

| テスト名 | 内容 |
|---|---|
| `alloc_basic_001` | `alloc(1)` で 1 セル確保、書き込み・読み取り |
| `alloc_basic_002` | `alloc(10)` で 10 セル確保、全セル書き込み・読み取り |
| `alloc_multi_001` | 複数回 `alloc` で異なるブロック確保、アドレスが重複しないこと |
| `alloc_free_reuse_001` | `alloc` → `free` → `alloc` で解放ブロックが再利用されること |
| `alloc_free_reuse_002` | サイズの異なるブロックの確保・解放・再確保 |
| `alloc_split_001` | 大きなフリーブロックの分割が正しく動作すること |
| `alloc_zero_size_001` | `alloc(0)` が最小ブロック (1セル) を返すこと |

### テスト方法

Whitespace VM のテストフレームワーク（`--std-ext debug` の `__trace` 相当）を使用して結果を検証:

```
# テスト: alloc(1) で確保した領域に書き込み・読み取り
push 1
call __rt_alloc       # ptr = alloc(1)
dup
push 42
store                 # heap[ptr] = 42
retrieve              # heap[ptr] → 42
push -10
swap
store                 # __trace(42)
```

## Phase 3 テスト: スタックフレーム統合

### 既存テストの回帰テスト

`--std-ext alloc` を有効にした状態で、既存の nospace テストを実行。

特に重要なテスト:
- `array-basic.ns` — 配列アクセス
- `array-reference.ns` — 参照・ポインタ
- `array-static.ns` — static 配列
- `scope-*` — スコープ関連
- `examples/` — 実用的なプログラム（fibonacci, qsort 等）

### 新規テスト

| テスト名 | 内容 | ファイル |
|---|---|---|
| `alloc_stack_frame_001` | 単純な関数呼び出しとローカル変数 | `passes/alloc-stack-frame.ns` |
| `alloc_recursive_001` | 再帰呼び出しでフレームが正しく確保・解放されること | `passes/alloc-recursive.ns` |
| `alloc_deep_recursion_001` | 深い再帰でもフレーム管理が正しいこと | `passes/alloc-deep-recursion.ns` |
| `alloc_nested_func_001` | ネストした関数のフレーム管理 | `passes/alloc-nested-func.ns` |

## Phase 4 テスト: `__alloc`/`__free` 組み込み関数

### nospace 統合テスト

| テスト名 | 内容 | ファイル |
|---|---|---|
| `builtin_alloc_basic_001` | `__alloc(1)` で 1 セル確保、`*ptr = 42; __trace(*ptr)` | `passes/builtin-alloc-basic.ns` |
| `builtin_alloc_array_001` | `__alloc(10)` で動的配列をシミュレート | `passes/builtin-alloc-array.ns` |
| `builtin_alloc_free_001` | `__alloc` → 使用 → `__free` → 別の `__alloc` で再利用確認 | `passes/builtin-alloc-free.ns` |
| `builtin_alloc_linked_list_001` | ポインタで連結リストを構築 | `passes/builtin-alloc-linked-list.ns` |
| `builtin_alloc_free_not_enabled_001` | `--std-ext alloc` なしで `__alloc` を使うとエラー | `fails/` |

### テストコード例

#### `builtin-alloc-basic.ns`

```nospace
func: main() {
  let: ptr(__alloc(3));
  *ptr = 10;
  *(ptr + 1) = 20;
  *(ptr + 2) = 30;
  __trace(*ptr);         # trace: 10
  __trace(*(ptr + 1));   # trace: 20
  __trace(*(ptr + 2));   # trace: 30
  __free(ptr);
  return: 0;
}
```

#### `builtin-alloc-linked-list.ns`

```nospace
# 連結リスト: ノード = [value, next_ptr]
func: new_node(val, next) {
  let: ptr(__alloc(2));
  *ptr = val;
  *(ptr + 1) = next;
  return: ptr;
}

func: sum_list(head) {
  let: total(0);
  let: curr(head);
  while: curr != 0 {
    total += *curr;
    curr = *(curr + 1);
  };
  return: total;
}

func: main() {
  let: list(0);
  list = new_node(3, list);
  list = new_node(2, list);
  list = new_node(1, list);
  # list: 1 → 2 → 3 → 0
  __trace(sum_list(list));  # trace: 6
  return: 0;
}
```

## テスト実行方法

### Unit テスト (Whitespace VM 直接)

```bash
cargo test -- alloc
```

### 統合テスト (nospace コンパイル→実行)

テストランナーに `--std-ext alloc` を渡す仕組みが必要。

既存の `test-manifest.yaml` に `std_ext: ["alloc"]` フィールドを追加:

```yaml
- name: builtin_alloc_basic_001
  source: passes/builtin-alloc-basic.ns
  std_ext: ["alloc", "debug"]  # debug も必要 (__trace を使うため)
```

### 回帰テスト

既存テストを `--std-ext alloc` 付きで再実行するテストターゲットを追加：

```bash
# 通常テスト
cargo test
# alloc 拡張付きで再実行
cargo test -- --ignored alloc
```

## テスト優先度

| 優先度 | テスト | 理由 |
|---|---|---|
| 高 | alloc/free 基本動作 | 機能の正しさの根幹 |
| 高 | スタックフレーム回帰 | 既存機能の動作保証 |
| 中 | 再帰・ネスト | エッジケース |
| 中 | 連結リスト等の応用 | 実用性の確認 |
| 低 | パフォーマンス | 初期段階では不要 |
