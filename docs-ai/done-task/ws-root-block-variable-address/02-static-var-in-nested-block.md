# 02: ブロック内からの static 変数参照の修正

関数内のブロックから同じ関数の static 変数を参照したとき、static 領域のアドレスを使うようにする（README の C）。

## 設計・方針

### 前提: `scope_depth` と `scope_offsets` の対応

- `IdentifierRef.scope_depth` は、参照位置のスコープから何段外のスコープに変数があるか（0 = 現在のスコープ）
- `CodeGenContext.scope_offsets` は関数スコープ（index 0）から現在のブロックまでのスタック。意味解析のスコープスタックと同じ段数で積まれる（`for` の init スコープも両方で 1 段積む: `semantic_analyzer/statement.rs:500-`、`compiler_ws/statement.rs:252`）
- よって、関数スコープ直下の変数は `scope_depth == scope_offsets.len() - 1`。`scope_depth >= scope_offsets.len()` は関数の外（親関数・グローバル）

### `get_var_info` の新しい判定（`context.rs:220-270`）

`is_global` の場合は変更なし。それ以外:

| 条件 | 結果 |
|---|---|
| 関数内で `owning_func_index == Some(owner)` | `layout.static_var_offset(owner, local_index)` があれば `Global` |
| 関数内で `owning_func_index == None` かつ `scope_depth == scope_offsets.len() - 1`（関数スコープ直下） | `layout.static_var_offset(current_func_index, local_index)` があれば `Global` |
| 上で見つからず `scope_depth >= scope_offsets.len()` | 従来どおり `Global`（`offset = local_index`）。下記「fallback」参照 |
| それ以外 | 従来どおり `Local`（`scope_offsets[len - 1 - scope_depth] + local_index`） |

変更点は 2 つ:

1. 自関数の static の検索を `scope_depth == 0` から「関数スコープ直下」に直す。ブロック内からの static 参照（C1・C3）が static 領域になり、ブロック変数（`scope_depth == 0`、`len >= 2`）を static と誤認しなくなる（C2）
2. `owning_func_index` がある場合は `scope_depth` の値に関係なく親関数の static を検索する。今は `scope_depth >= len` を条件にしているが、意味解析は関数境界を越えた static 参照にしか `owning_func_index` を付けないので、`scope_depth` の条件は不要。結果は今と変わらない（境界を越えれば必ず `scope_depth >= len`）が、条件を `owning_func_index` だけにして判定を単純にする

fallback（`scope_depth >= len` で static でもない）は、正しい入力では通らない想定だが、README「対象外」のルートブロック内の関数からブロック変数を参照するケースで通る可能性がある。その扱いは別タスク（[nested-block-function-and-static](../../task/nested-block-function-and-static.md)）で決めるため、本タスクでは挙動を変えない。

関数スコープ直下の static 以外の変数（`let:`）は `static_var_offset` が `None` なので `Local` のまま。

static 初期化コンテキスト（`enter_function_for_static_init`）も `current_func_index` と、関数スコープを index 0 とする `scope_offsets`（[01](01-init-block-region.md) で `[range.start]` にする）を持つので、同じ判定で static 初期化子のブロックから同じ関数の static を参照できる（C3）。

### バグのコメント

`get_var_info` の判定箇所に、原因（`scope_depth == 0` を「関数スコープ直下」と取り違えていた）と解決策を書く。

## テスト

large テスト（`resources/tests/passes/scope/`、`success_io`）:

| ファイル | 内容 | 期待 stdout |
|---|---|---|
| `static_in_block_001` | C1 + C2。ブロック・`if`・`while`・`for` の中から（static 宣言は関数スコープ直下のみ。ブロック内の static 宣言は対象外の別バグ） static を読み書きし、2 回呼んで値が持続することを確認。ブロック変数と static のスロット番号が一致する配置も含める。グローバル変数を 1 個以上置き、グローバル領域の先頭と static が重ならないようにする | run と同じ値 |
| `static_in_block_nested_func_001` | ネストした関数のブロック内から親関数の static を参照（`owning_func_index` 経路の回帰） | run と同じ値 |

C3 は [01](01-init-block-region.md) の `func_static_init_block_var_001` で確認する。
