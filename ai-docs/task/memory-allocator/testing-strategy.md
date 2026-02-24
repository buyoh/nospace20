# テスト計画

## テスト方針

メモリアロケータの実装は非常に複雑であるため、テストを **4 層** に分けて実施する。特に、アロケータ実装単体を nospace コンパイラパイプラインから完全に分離してテスト可能な仕組みを構築する。

### テスト 4 層構造

| 層 | 名称 | 対象 | 依存 | 場所 |
|---|---|---|---|---|
| **L1** | 分離テスト | アロケータサブルーチン単体 | alloc_runtime + WhitespaceVM のみ | `resources/tests_alloc/` + `tests/alloc_test.rs` |
| **L2** | alloc_runtime Unit テスト | コード生成ロジック | compiler_ws モジュール内 | `src/compiler_ws/alloc_runtime.rs` 内の `#[cfg(test)]` |
| **L3** | 統合テスト | nospace → WS → 実行の E2E | nospace 完全パイプライン | `resources/tests/` |
| **L4** | 回帰テスト | 既存テストの `--std-ext alloc` 有効時の動作 | nospace 完全パイプライン | 既存の `resources/tests/` テストに `std_ext` 追加 |

**L1 (分離テスト) が最も重要**。設計の詳細は [isolated-testing.md](isolated-testing.md) を参照。

## L1: 分離テスト（JSON ミニ言語）

### 概要

アロケータ操作と検証を記述する JSON ベースの独自テスト言語を使用し、nospace コンパイラを一切経由せずにアロケータをテストする。

**依存する最小構成**:
- `compiler_ws/alloc_runtime` — アロケータの WS コード生成（テスト対象）
- `whitespace/interpreter` — WS VM（テスト実行環境）
- テスト用ミニコンパイラ（`tests/alloc_test.rs` 内に実装）

**依存しないもの**: token_parser, tree_parser, semantic_analyzer, interpreter, その他 compiler_ws モジュール

### テスト一覧 (Phase 2)

| テスト名 | カテゴリ | 検証内容 |
|---|---|---|
| `alloc_basic_001` | basic | `alloc(1)` で 1 セル確保、書き込み・読み取り |
| `alloc_basic_002` | basic | `alloc(10)` で 10 セル確保、全セル書き込み・読み取り |
| `alloc_multi_001` | basic | 複数回 `alloc` で非重複を検証 |
| `alloc_free_reuse_001` | basic | `alloc` → `free` → `alloc` で再利用確認 |
| `alloc_free_reuse_002` | basic | 異なるサイズの確保・解放・再確保 |
| `alloc_split_001` | basic | 大きなフリーブロックの分割 |
| `alloc_zero_size_001` | basic | `alloc(0)` → 最小ブロック確保 |
| `fsba_class_reuse_001` | fsba | 同一サイズクラスの alloc/free/alloc 再利用 |
| `fsba_different_class_001` | fsba | 異なるサイズクラスの独立管理 |
| `fsba_roundup_001` | fsba | サイズ切り上げの正しさ |
| `fsba_large_fallback_001` | fsba | 32 セル超の汎用フォールバック |
| `repeated_alloc_free_001` | stress | 100 回の alloc/free 繰り返し |
| `many_small_allocs_001` | stress | 多数の小ブロック確保 |
| `heap_top_growth_001` | metadata | ALLOC_HEAP_TOP の成長を検証 |
| `free_list_structure_001` | metadata | フリーリストの構造を検証 |

### テスト実行

```bash
# 分離テストのみ実行
cargo test --test alloc_test

# 特定のテストを実行
cargo test --test alloc_test -- alloc_basic_001

# FSBA テストのみ
cargo test --test alloc_test -- fsba
```

詳細な設計（JSON スキーマ、ミニコンパイラ、ディレクトリ構成）は [isolated-testing.md](isolated-testing.md) を参照。

## L2: alloc_runtime Unit テスト

`src/compiler_ws/alloc_runtime.rs` 内の `#[cfg(test)] mod tests` で実施。

テスト観点:
- 初期化コードが正しい命令列を生成すること
- ラベル名が衝突しないこと
- `global_heap_size` の変更に追従してアドレス計算が正しいこと

これは Rust レベルの unit テストであり、WS VM 実行は伴わない。

## L3: 統合テスト — nospace E2E

### Phase 3 テスト: スタックフレーム統合

`--std-ext alloc` を有効にした nospace コードの E2E テスト。

| テスト名 | 内容 | ファイル |
|---|---|---|
| `alloc_stack_frame_001` | 単純な関数呼び出しとローカル変数 | `passes/alloc-stack-frame.ns` |
| `alloc_recursive_001` | 再帰呼び出しでフレームが正しく確保・解放されること | `passes/alloc-recursive.ns` |
| `alloc_deep_recursion_001` | 深い再帰でもフレーム管理が正しいこと | `passes/alloc-deep-recursion.ns` |
| `alloc_nested_func_001` | ネストした関数のフレーム管理 | `passes/alloc-nested-func.ns` |

### Phase 4 テスト: `__alloc`/`__free` 組み込み関数

| テスト名 | 内容 | ファイル |
|---|---|---|
| `builtin_alloc_basic_001` | `__alloc(1)` で 1 セル確保、書き込み・読み取り | `passes/builtin-alloc-basic.ns` |
| `builtin_alloc_array_001` | `__alloc(10)` で動的配列をシミュレート | `passes/builtin-alloc-array.ns` |
| `builtin_alloc_free_001` | `__alloc` → 使用 → `__free` → 再確保 | `passes/builtin-alloc-free.ns` |
| `builtin_alloc_linked_list_001` | ポインタで連結リストを構築 | `passes/builtin-alloc-linked-list.ns` |
| `builtin_alloc_free_not_enabled_001` | `--std-ext alloc` なしで `__alloc` 使用→エラー | `fails/` |

### nospace テストコード例

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

### test-manifest.yaml 統合

`resources/tests/test-manifest.yaml` に `std_ext` フィールドを追加:

```yaml
- name: builtin_alloc_basic_001
  type: success
  path: passes/builtin-alloc-basic
  std_ext: ["alloc", "debug"]  # debug も必要 (__trace を使うため)
```

## L4: 回帰テスト

### 既存テストの `--std-ext alloc` 対応

`--std-ext alloc` を有効にした状態で、既存の nospace テストが引き続きパスすることを確認する。

特に重要なテスト:
- `array-basic.ns` — 配列アクセス
- `array-reference.ns` — 参照・ポインタ
- `array-static.ns` — static 配列
- `scope-*` — スコープ関連
- `examples/` — 実用的なプログラム（fibonacci, qsort 等）

### 実行方法

```bash
# 通常テスト
cargo test
# alloc 拡張付きで回帰テスト実行
cargo test -- --ignored alloc_regression
```

## テスト優先度

| 優先度 | テスト層 | テスト | 理由 |
|---|---|---|---|
| **最高** | L1 | alloc/free 基本動作 | 機能の正しさの根幹。分離テストで早期検証 |
| **最高** | L1 | FSBA クラス分け・再利用 | 二層アーキテクチャの正しさ。分離テストで早期検証 |
| 高 | L2 | コード生成 unit テスト | 命令列生成の正しさ |
| 高 | L4 | スタックフレーム回帰 | 既存機能の動作保証 |
| 中 | L1 | ストレス・メタデータテスト | エッジケースと内部状態 |
| 中 | L3 | 再帰・ネスト | E2E でのエッジケース |
| 中 | L3 | 連結リスト等の応用 | 実用性の確認 |
| 低 | - | パフォーマンス | 初期段階では不要 |

## Phase ごとのテスト実装順序

| Phase | テスト層 | 実装するテスト |
|---|---|---|
| Phase 2 | L1, L2 | 分離テスト全件 + alloc_runtime unit テスト |
| Phase 3 | L3, L4 | スタックフレームテスト + 回帰テスト |
| Phase 4 | L3 | `__alloc`/`__free` 統合テスト |
