# コアアロケータのアルゴリズムとデータ構造

## アルゴリズム選定

### 候補比較

| アルゴリズム | 実装複雑度 | フラグメンテーション | free 対応 | Whitespace 適合性 |
|---|---|---|---|---|
| バンプアロケータ | ◎ 非常に簡単 | × 解放不可 | × | ◎ |
| フリーリスト (First-Fit) | ○ 中程度 | △ | ○ | ○ |
| 固定サイズブロック (FSBA) | ○ 中程度 | ○ (内部frag小) | ○ | ◎ |
| バディアロケータ | × 複雑 | ○ | ○ | △ |

### 採用: 二層アーキテクチャ — FSBA + 汎用フリーリスト (First-Fit) + バンプフォールバック

**構成**:
- **第 1 層: 固定サイズブロックアロケータ (FSBA)** — 小さなサイズ (≤32 セル) の高速割り当て。サイズクラス (2, 4, 8, 16, 32) ごとにフリーリストを持ち、O(1) で alloc/free
- **第 2 層: 汎用 First-Fit + バンプ** — 大きなサイズ (>32 セル) のフォールバック。フリーリストを First-Fit で走査し、見つからなければバンプ拡張

**理由**:
- `__free` をサポートするにはフリーリストが必要
- Whitespace のスタックマシンで実装可能な複雑度
- FSBA によりスタックフレームなど頻繁な小サイズ割り当てが O(1) で完了
- 同一サイズクラスのブロックは完璧に再利用でき、外部フラグメンテーションが発生しない
- フリーリストに適合するブロックがない場合、ヒープ末尾をバンプ拡張

**詳細設計**: [fixed-size-block-allocator.md](fixed-size-block-allocator.md) を参照

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

アロケータは以下のメタデータアドレスを使用する:

| アドレス | 名前 | 説明 |
|---|---|---|
| 5 | `ALLOC_FREE_HEAD` | 汎用フリーリストの先頭ブロックアドレス (0 = 空) |
| 6 | `ALLOC_HEAP_TOP` | マネージドヒープの現在の末尾（バンプ拡張用） |
| 7 | `FSBA_TABLE_PTR` | FSBA フリーリストテーブルへのポインタ |

現在アドレス 5-7 は予約済み未使用領域のため、ここに配置する。

FSBA テーブル自体はマネージドヒープの先頭に配置される（詳細は [fixed-size-block-allocator.md](fixed-size-block-allocator.md) および [heap-layout.md](heap-layout.md) を参照）。

## 擬似コード

alloc/free の全体フロー（FSBA 統合版）は [fixed-size-block-allocator.md](fixed-size-block-allocator.md) を参照。
以下は汎用アロケータ (第 2 層) の擬似コードである。

### general_alloc(total) → ptr

total は既にヘッダー込みの値（`max(requested_size + 1, 2)` 済み）で、かつ total > 32 のケース。

```
function general_alloc(total):
    // 汎用フリーリストを First-Fit で探索
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

### general_free(ptr)

total > 32 のブロック、または FSBA サイズクラスに該当しないブロック用。

```
function general_free(ptr):
    block = ptr - 1
    // 汎用フリーリストの先頭に追加 (LIFO)
    heap[block + 1] = heap[ALLOC_FREE_HEAD]
    heap[ALLOC_FREE_HEAD] = block
```

### 初期化

FSBA テーブルをマネージドヒープの先頭に配置し、その直後からバンプ拡張を開始する。

```
function init_allocator():
    managed_start = GLOBAL_PTR + global_heap_size

    // FSBA テーブル初期化
    heap[FSBA_TABLE_PTR] = managed_start   // アドレス 7
    for i in 0..FSBA_CLASS_COUNT:
        heap[managed_start + i] = 0        // 各サイズクラスのフリーリスト空

    // 汎用アロケータ初期化
    heap[ALLOC_FREE_HEAD] = 0              // 汎用フリーリスト空
    heap[ALLOC_HEAP_TOP] = managed_start + FSBA_CLASS_COUNT  // テーブル直後
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

### 複雑度の見積り（二層統合）

| 処理 | 概算命令数 |
|---|---|
| total 計算 (max, +1) | ~10 |
| FSBA サイズクラス選択 (5 段カスケード) | ~25 |
| FSBA alloc (フリーリスト pop / バンプ) | ~20 |
| 汎用 alloc (First-Fit ループ + バンプ) | ~50 |
| **alloc 合計** | **~105 命令** |
| free: ヘッダ読取 + サイズクラス判定 | ~30 |
| free: FSBA push / 汎用 push | ~10 |
| **free 合計** | **~40 命令** |

※ FSBA ヒット時の実行パスは alloc ~35 命令、free ~20 命令程度でショートカットされる。

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
