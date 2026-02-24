# 固定サイズブロックアロケータ (FSBA) 設計

## 概要

固定サイズブロックアロケータ (Fixed-Size Block Allocator, FSBA) は、要求されたサイズを事前に定義されたサイズクラスに切り上げて割り当てる方式である。

**基本的な発想**: 要求された量ぴったりのメモリを返す代わりに、いくつかのブロックサイズクラスを決め、割り当てサイズを次のブロックサイズに切り上げる。

**例**: サイズクラスが 2, 4, 8, 16, 32 セルの場合:
- 1 セルの要求 → 2 セルのブロックを返す
- 3 セルの要求 → 4 セルのブロックを返す
- 5 セルの要求 → 8 セルのブロックを返す
- 20 セルの要求 → 32 セルのブロックを返す
- 33 セル以上 → 汎用アロケータ (First-Fit) にフォールバック

## 利点

| 特性 | FSBA | 汎用 First-Fit |
|---|---|---|
| alloc 時間計算量 | O(1) (フリーリストヒット時) | O(n) (フリーリスト走査) |
| free 時間計算量 | O(1) | O(1) |
| ブロック分割 | 不要 | 必要 |
| ブロック結合 | 不要 | 将来的に必要 |
| 内部フラグメンテーション | あり（切り上げ分） | なし |
| 外部フラグメンテーション | サイズクラス内で発生しない | 発生する |
| 実装複雑度 | 低い | 中程度 |

## 二層アーキテクチャ

FSBA は汎用アロケータのフロントエンドとして機能する:

```
alloc(size) リクエスト
        │
        ▼
┌──────────────────────────┐
│ サイズクラス選択           │
│ total = max(size+1, 2)   │
│ total <= MAX_CLASS_SIZE ? │
└────────┬─────────────────┘
         │
   ┌─────┴──────┐
   │ Yes        │ No (oversized)
   ▼            ▼
┌────────┐  ┌──────────────────────┐
│ FSBA   │  │ 汎用アロケータ        │
│ O(1)   │  │ First-Fit + バンプ    │
│        │  │ O(n)                 │
└────────┘  └──────────────────────┘
```

```
free(ptr)
        │
        ▼
┌─────────────────────────────┐
│ ブロックヘッダからサイズ取得   │
│ block_size = heap[ptr - 1]  │
│ サイズクラスに該当するか？    │
└────────┬────────────────────┘
         │
   ┌─────┴──────┐
   │ Yes        │ No
   ▼            ▼
┌────────┐  ┌──────────────────────┐
│ FSBA   │  │ 汎用アロケータ        │
│ free   │  │ free (LIFO push)     │
│ O(1)   │  │ O(1)                 │
└────────┘  └──────────────────────┘
```

## サイズクラス定義

### ブロックサイズ（ヘッダー含む）

nospace の Whitespace ターゲットでは、ヒープの 1 スロットが 1 セル (i64) である。各ブロックは 1 セルのヘッダーを含む。

| クラス | ブロックサイズ | ユーザーサイズ | 典型的な用途 |
|--------|---------------|---------------|-------------|
| 0 | 2 セル | 1 セル | 単一変数、スカラー |
| 1 | 4 セル | 3 セル | 小さな構造体、短い配列 |
| 2 | 8 セル | 7 セル | 関数フレーム（小） |
| 3 | 16 セル | 15 セル | 関数フレーム（中） |
| 4 | 32 セル | 31 セル | 関数フレーム（大）、中規模配列 |

**定数**: `FSBA_CLASS_COUNT = 5`

### サイズクラスの選択ロジック

```
requested_size → total = max(requested_size + 1, 2)

total <= 2  → クラス 0 (ブロック  2 セル)
total <= 4  → クラス 1 (ブロック  4 セル)
total <= 8  → クラス 2 (ブロック  8 セル)
total <= 16 → クラス 3 (ブロック 16 セル)
total <= 32 → クラス 4 (ブロック 32 セル)
total > 32  → 汎用アロケータへフォールバック
```

実例:

| `__alloc(n)` | total (n+1) | サイズクラス | 実際のブロックサイズ | 内部フラグメンテーション |
|---|---|---|---|---|
| `__alloc(0)` | 2 | 0 | 2 | 0 セル |
| `__alloc(1)` | 2 | 0 | 2 | 0 セル |
| `__alloc(2)` | 3 | 1 | 4 | 1 セル |
| `__alloc(3)` | 4 | 1 | 4 | 0 セル |
| `__alloc(7)` | 8 | 2 | 8 | 0 セル |
| `__alloc(10)` | 11 | 3 | 16 | 5 セル |
| `__alloc(31)` | 32 | 4 | 32 | 0 セル |
| `__alloc(32)` | 33 | - | 33 (汎用) | 0 セル |

### スタックフレームとの相性

関数のローカル変数数にもとづくフレームサイズ別の分類:

| ローカル変数数 | フレームサイズ | サイズクラス | 備考 |
|---|---|---|---|
| 1 | 1 | 0 (2 セル) | 最小フレーム |
| 2-3 | 2-3 | 1 (4 セル) | 小さな関数 |
| 4-7 | 4-7 | 2 (8 セル) | 一般的な関数 |
| 8-15 | 8-15 | 3 (16 セル) | やや大きな関数 |
| 16-31 | 16-31 | 4 (32 セル) | 大きな関数 |
| 32+ | 32+ | 汎用 | 巨大フレーム / 大配列 |

多くの関数はフレームサイズが 1-15 セル程度であるため、クラス 0-3 でカバーできる。

## FSBA フリーリストテーブル

### メタデータ配置

各サイズクラスに対してフリーリストの先頭ポインタを保持する必要がある。5 クラス分で 5 セル。

**方式**: マネージドヒープの先頭にテーブルを配置し、予約アドレス 7 (`FSBA_TABLE_PTR`) でテーブルの位置を参照する。

```
予約アドレス:
  7: FSBA_TABLE_PTR → マネージドヒープ内のテーブル先頭を指す

マネージドヒープ:
  heap[TABLE + 0] = クラス 0 フリーリスト先頭 (0 = 空)
  heap[TABLE + 1] = クラス 1 フリーリスト先頭
  heap[TABLE + 2] = クラス 2 フリーリスト先頭
  heap[TABLE + 3] = クラス 3 フリーリスト先頭
  heap[TABLE + 4] = クラス 4 フリーリスト先頭
  ─── テーブル終了 ───
  (以降: 実際の割り当てブロック)
```

### 初期化

```
function init_allocator():
    managed_start = GLOBAL_PTR + global_heap_size

    // FSBA テーブル配置
    heap[FSBA_TABLE_PTR] = managed_start    // アドレス 7 にテーブル位置を格納
    heap[managed_start + 0] = 0             // クラス 0 フリーリスト空
    heap[managed_start + 1] = 0             // クラス 1 フリーリスト空
    heap[managed_start + 2] = 0             // クラス 2 フリーリスト空
    heap[managed_start + 3] = 0             // クラス 3 フリーリスト空
    heap[managed_start + 4] = 0             // クラス 4 フリーリスト空

    // 汎用アロケータ
    heap[ALLOC_FREE_HEAD] = 0
    heap[ALLOC_HEAP_TOP] = managed_start + FSBA_CLASS_COUNT  // テーブル直後
```

## 擬似コード

### alloc(requested_size) → ptr

```
function alloc(requested_size):
    total = max(requested_size + 1, 2)

    // サイズクラス選択 (カスケード比較)
    if total <= 2:
        return fsba_alloc(0, 2)
    else if total <= 4:
        return fsba_alloc(1, 4)
    else if total <= 8:
        return fsba_alloc(2, 8)
    else if total <= 16:
        return fsba_alloc(3, 16)
    else if total <= 32:
        return fsba_alloc(4, 32)
    else:
        return general_alloc(total)   // 汎用アロケータ (既存の First-Fit + バンプ)

function fsba_alloc(class_index, class_size):
    table_ptr = heap[FSBA_TABLE_PTR]
    free_head_addr = table_ptr + class_index
    free_head = heap[free_head_addr]

    if free_head != 0:
        // フリーリストからポップ
        block = free_head
        heap[free_head_addr] = heap[block + 1]   // next を新しい先頭に
        // block_size はすでに class_size のはず（検証不要）
        return block + 1

    // フリーリスト空 → バンプ拡張で新規ブロック確保
    block = heap[ALLOC_HEAP_TOP]
    heap[block] = class_size                      // ヘッダーにサイズ書き込み
    heap[ALLOC_HEAP_TOP] = block + class_size     // バンプポインタ進行
    return block + 1
```

### free(ptr)

```
function free(ptr):
    block = ptr - 1
    block_size = heap[block]

    // サイズクラス判定
    class_index = size_to_class(block_size)

    if class_index >= 0:
        // FSBA フリーリストにプッシュ
        table_ptr = heap[FSBA_TABLE_PTR]
        free_head_addr = table_ptr + class_index
        heap[block + 1] = heap[free_head_addr]    // 現在の先頭を next に
        heap[free_head_addr] = block              // ブロックを新しい先頭に
    else:
        // 汎用フリーリストにプッシュ
        heap[block + 1] = heap[ALLOC_FREE_HEAD]
        heap[ALLOC_FREE_HEAD] = block

function size_to_class(block_size):
    // 正確なサイズクラスのみ該当
    if block_size == 2:  return 0
    if block_size == 4:  return 1
    if block_size == 8:  return 2
    if block_size == 16: return 3
    if block_size == 32: return 4
    return -1  // size class に該当しない → 汎用
```

## Whitespace 実装の詳細

### サイズクラス選択の命令列

Whitespace にはテーブルルックアップがないため、カスケード比較で実装する:

```
# 入力: スタックトップに total
# 出力: 適切な fsba_alloc または general_alloc へジャンプ

    dup
    push 2
    sub
    jn _class0_or_less     # total <= 2 → impossible (total >= 2)
    jz _class0             # total == 2 → class 0

    dup
    push 4
    sub
    jn _class1             # total <= 4 → class 1 (2 < total <= 4)
    jz _class1             # total == 4 → class 1

    dup
    push 8
    sub
    jn _class2             # total <= 8 → class 2
    jz _class2

    dup
    push 16
    sub
    jn _class3             # total <= 16 → class 3
    jz _class3

    dup
    push 32
    sub
    jn _class4             # total <= 32 → class 4
    jz _class4

    jmp _general_alloc     # total > 32 → 汎用アロケータ
```

各 `_classN` ラベルの処理は同パターン:
1. テーブルからフリーリスト先頭を取得
2. 先頭が非ゼロならフリーリストからポップして返却
3. ゼロならバンプ拡張

### free のサイズクラス判定

```
# 入力: スタックトップに block (= ptr - 1)
# heap[block] からブロックサイズを取得し、クラスを判定

    dup
    retrieve               # block_size = heap[block]

    dup
    push 2
    sub
    jz _free_class0        # block_size == 2

    dup
    push 4
    sub
    jz _free_class1        # block_size == 4

    dup
    push 8
    sub
    jz _free_class2        # block_size == 8

    dup
    push 16
    sub
    jz _free_class3        # block_size == 16

    dup
    push 32
    sub
    jz _free_class4        # block_size == 32

    pop                    # block_size を破棄
    jmp _free_general      # 汎用フリーリストへ
```

### 命令数の見積り

| サブルーチン | 処理 | 概算命令数 |
|---|---|---|
| alloc: total 計算 | max(size+1, 2) | ~10 |
| alloc: サイズクラス選択 | 5 段カスケード比較 | ~25 |
| alloc: fsba_alloc (5 クラス共通) | フリーリスト pop / バンプ拡張 | ~20 |
| alloc: general_alloc | First-Fit ループ + バンプ | ~50 |
| **alloc 合計** | | **~105 命令** |
| free: ブロックヘッダ読取 | | ~5 |
| free: サイズクラス判定 | 5 段比較 | ~25 |
| free: fsba_free / general_free | フリーリスト push | ~10 |
| **free 合計** | | **~40 命令** |

FSBA なしの場合と比較:
- **alloc**: ~70 → ~105 命令 (+35、ただし FSBA ヒット時は ~35 命令で完了)
- **free**: ~10 → ~40 命令 (+30、サイズクラス判定のオーバーヘッド)

サイズクラスにヒットする場合の実行時命令数は大幅に少ない（フリーリストに空きがあれば分岐なしで O(1)）。

## 設計上の考慮事項

### サイズクラスの拡張性

初期実装は 5 クラス (2, 4, 8, 16, 32) だが、将来的に追加可能:
- クラス追加時は `FSBA_CLASS_COUNT` 変更 + カスケード比較の段数追加
- テーブルサイズが増えるだけで、既存コードへの影響は最小

### ヘッダーの互換性

FSBA と汎用アロケータは同じブロックヘッダー形式を共有:
- `heap[block] = block_size` (ヘッダー含むサイズ)
- `heap[block + 1]` = フリー時は next ポインタ、割当時はユーザーデータ

この統一により、free 時にブロックサイズを見てディスパッチするだけで正しく処理される。

### バンプ拡張の共有

FSBA とバンプの新規ブロック確保は両方とも `ALLOC_HEAP_TOP` を使用する。これにより:
- メモリ空間を一元管理
- FSBA 用と汎用用でヒープを分割する必要がない
- 全てのブロックがアドレス空間上で連続的に配置される

### 内部フラグメンテーション

最悪ケース: `__alloc(17)` → total=18 → クラス 4 (32 セル) → 14 セルの無駄 (44%)

一般的なケース: スタックフレームは固定サイズなので、同じ関数の繰り返し呼び出しでは解放ブロックを完璧に再利用できる。

### alloc(0) の挙動

`alloc(0)` → total = max(0+1, 2) = 2 → クラス 0 → 2 セルブロック確保。
ユーザーに返るポインタは有効だが、書き込み可能なセルは実質 1。
