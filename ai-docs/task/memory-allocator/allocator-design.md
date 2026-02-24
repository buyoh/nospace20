# コアアロケータのアルゴリズムとデータ構造

## アルゴリズム選定

### 候補比較

| アルゴリズム | 実装複雑度 | フラグメンテーション | free 対応 | Whitespace 適合性 |
|---|---|---|---|---|
| バンプアロケータ | ◎ 非常に簡単 | × 解放不可 | × | ◎ |
| フリーリスト (First-Fit) | ○ 中程度 | △ | ○ | ○ |
| バディアロケータ | × 複雑 | ○ | ○ | △ |

### 採用: フリーリスト (First-Fit) + バンプフォールバック

**理由**:
- `__free` をサポートするにはフリーリストが必要
- Whitespace のスタックマシンで実装可能な複雑度
- First-Fit は実装がシンプルで、小規模プログラムには十分
- フリーリストに適合するブロックがない場合、ヒープ末尾をバンプ拡張

## ブロック構造

各アロケーションブロックは 1 セルのヘッダーを持つ:

```
┌─────────────────────────────────────────┐
│ heap[block]     = block_size (ヘッダ含む) │  ← ブロックヘッダー
│ heap[block + 1] = user_data[0]           │  ← alloc() が返すポインタ
│ heap[block + 2] = user_data[1]           │
│ ...                                      │
│ heap[block + N] = user_data[N-1]         │
└─────────────────────────────────────────┘
```

- `block_size` = リクエストサイズ + 1 (ヘッダー分)
- `alloc(n)` は `block + 1` を返す
- `free(ptr)` は `ptr - 1` でヘッダーを取得

### フリーリスト上のブロック

解放されたブロックでは、ユーザーデータ領域の先頭を次のフリーブロックへのポインタとして使用:

```
┌──────────────────────────────────────────┐
│ heap[block]     = block_size             │  ← サイズ保持
│ heap[block + 1] = next_free_block (0=末尾)│  ← 次のフリーブロック
│ heap[block + 2] = (未使用)               │
│ ...                                      │
└──────────────────────────────────────────┘
```

**最小ブロックサイズ**: 2 セル（ヘッダー 1 + next ポインタ 1）。
`alloc(0)` は `alloc(1)` と同等に扱い、最小 2 セルのブロックを確保する。

## メタデータ

アロケータは以下の 2 つのメタデータアドレスを使用する:

| アドレス | 名前 | 説明 |
|---|---|---|
| 5 | `ALLOC_FREE_HEAD` | フリーリストの先頭ブロックアドレス (0 = 空) |
| 6 | `ALLOC_HEAP_TOP` | マネージドヒープの現在の末尾（バンプ拡張用） |

現在アドレス 5-7 は予約済み未使用領域のため、ここに配置する。

## 擬似コード

### alloc(requested_size) → ptr

```
function alloc(requested_size):
    total = max(requested_size + 1, 2)   // ヘッダー 1 セル + 最小 1 セル

    // フリーリストを First-Fit で探索
    prev_addr = &ALLOC_FREE_HEAD  // メタデータアドレス(5)を prev として扱う
    curr = heap[ALLOC_FREE_HEAD]

    while curr != 0:
        curr_size = heap[curr]
        if curr_size >= total:
            // ブロック分割の検討
            if curr_size >= total + 2:  // 残余が最小ブロック(2セル)以上
                // 分割: 後半部分を新しいフリーブロックとして残す
                remainder = curr + total
                heap[remainder] = curr_size - total
                heap[remainder + 1] = heap[curr + 1]
                heap[curr] = total
                // prev の next を remainder に更新
                heap[prev_addr_next] = remainder
            else:
                // 分割せずそのまま使用
                // prev の next を curr の next に更新
                heap[prev_addr_next] = heap[curr + 1]
            return curr + 1

        // 次のフリーブロックへ
        prev_addr = curr
        prev_addr_next = curr + 1
        curr = heap[curr + 1]

    // フリーリストに適合ブロックなし → バンプ拡張
    ptr = heap[ALLOC_HEAP_TOP]
    heap[ptr] = total
    heap[ALLOC_HEAP_TOP] = ptr + total
    return ptr + 1
```

### free(ptr)

```
function free(ptr):
    block = ptr - 1
    // フリーリストの先頭に追加 (LIFO)
    heap[block + 1] = heap[ALLOC_FREE_HEAD]
    heap[ALLOC_FREE_HEAD] = block
```

### 初期化

```
function init_allocator():
    heap[ALLOC_FREE_HEAD] = 0  // フリーリスト空
    heap[ALLOC_HEAP_TOP] = GLOBAL_PTR + global_heap_size  // グローバル領域直後
```

## Whitespace サブルーチンとしての実装

### 呼び出し規約

| サブルーチン | 入力スタック | 出力スタック | ラベル |
|---|---|---|---|
| `__runtime_alloc` | `[..., size]` | `[..., ptr]` | `__rt_alloc` |
| `__runtime_free` | `[..., ptr]` | `[...]` | `__rt_free` |

両サブルーチンは `Call`/`Return` 命令で呼び出す。引数と戻り値はデータスタック経由。

### alloc の Whitespace 実装戦略

alloc は以下の制御フローを必要とする:
- ループ (フリーリスト走査)
- 条件分岐 (サイズ比較、ブロック分割判定)

Whitespace には `JumpIfZero` と `JumpIfNegative` があるため、条件分岐は以下のパターンで実現:

```
# if A >= B (A - B >= 0):
push A
push B
sub         # stack: [A - B]
jn else     # A - B < 0 なら else へ
# then ブランチ
jmp end
# else: ...
# end: ...
```

### ループパターン

```
# while curr != 0:
loop_start:
  # curr をスタックに持ってくる (heap 読み込みなど)
  dup
  jz loop_end    # curr == 0 なら終了
  # ループ本体
  jmp loop_start
loop_end:
```

### 複雑度の見積り

alloc サブルーチンの命令数概算:

| 処理 | 概算命令数 |
|---|---|
| total 計算 (max, +1) | ~10 |
| フリーリスト走査ループ初期化 | ~5 |
| ループ本体 (サイズ比較 + 条件分岐) | ~20 |
| ブロック分割処理 | ~15 |
| フリーリストからの切り離し | ~10 |
| バンプフォールバック | ~10 |
| **合計** | **~70 命令** |

free サブルーチンは ~10 命令で実装可能。

## 将来の改善

### ブロック結合 (Coalescing)

初期実装ではブロック結合を行わない。将来的に以下を検討:

1. **アドレス順ソートのフリーリスト**: free 時にアドレス順で挿入し、隣接ブロックを結合
2. **フッターの追加**: 各ブロック末尾にサイズを格納し、前方ブロックとの結合を O(1) で実現

### ブロック分割の最小サイズ

初期実装: 分割後の残余が 2 セル以上なら分割。閾値は調整可能。

### デバッグ支援

`--std-ext alloc` と `--std-ext debug` の併用時:
- 解放済みブロックへのアクセス検出（ポイズニング）
- ダブルフリー検出
- リーク検出（プログラム終了時にフリーリスト以外のブロック一覧）
