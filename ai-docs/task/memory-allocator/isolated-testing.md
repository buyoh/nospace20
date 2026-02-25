# メモリアロケータの分離テスト設計

## 目的

メモリアロケータの実装は複雑であるため、nospace コンパイラパイプライン全体（構文解析・意味解析・インタプリタ）から分離して、アロケータ単体でテスト可能な仕組みを構築する。

## 設計方針

### 基本思想: 検証特化

テスト記述をできるだけ簡潔にするため、操作を最小限に絞る。

- **`alloc` が自動初期化する**: 確保した各要素にグローバルカウンタの連番値を書き込む。手動 `store` は不要
- **`load_print` は全要素出力**: offset 指定なし。確保領域のすべての値を出力する
- **出力は完全に決定的**: カウンタは 1 から始まり `alloc` ごとに size 分進むため、期待出力がテスト仕様から一意に決まる

### 分離テストの原則

1. **nospace パイプラインに非依存**: token_parser, tree_parser, semantic_analyzer, interpreter に一切依存しない
2. **依存する最小構成**:
   - `compiler_ws/alloc_runtime` — アロケータの WS コード生成（テスト対象）
   - `whitespace/interpreter` — WS VM（テスト実行環境）
   - テスト用ミニコンパイラ — JSON テスト仕様 → WS 命令列の変換
3. **JSON ベースのテスト記述**: アロケータ操作と期待結果を JSON で宣言的に記述

### 依存関係

```
tests/alloc_test.rs
  ├── nospace20::compiler_ws::alloc_runtime  ← テスト対象
  └── nospace20::whitespace::WhitespaceVM    ← 実行環境

resources/tests_alloc/*.test.json           ← テストデータ
```

**依存しないもの**: `token_parser`, `tree_parser`, `semantic_analyzer`, `interpreter`, `compiler_ws` の alloc_runtime 以外

## JSON テスト仕様フォーマット

### 基本構造

```json
{
  "description": "テストの説明",
  "config": {
    "global_heap_size": 0,
    "max_steps": 100000
  },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 3 },
    { "op": "load_print", "var": "p1" },
    { "op": "free", "var": "p1" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "1\n2\n3\n"
  }
}
```

### フィールド定義

| フィールド | 型 | 必須 | 説明 |
|---|---|---|---|
| `description` | string | No | テストの説明 |
| `config` | object | No | テスト設定 |
| `config.global_heap_size` | integer | No | グローバル変数領域のサイズ（デフォルト: 0） |
| `config.max_steps` | integer | No | VM 最大実行ステップ数（デフォルト: 100000） |
| `vars` | string[] | Yes | テスト内で使用する変数名のリスト |
| `steps` | object[] | Yes | 実行する操作のシーケンス |
| `check` | object | Yes | 期待結果の検証方法 |

### 操作 (Operations)

#### `alloc` — メモリ確保 + 自動初期化

```json
{ "op": "alloc", "var": "p1", "size": 3 }
```

`__rt_alloc(size)` を呼び出し、返されたポインタを変数 `var` に格納する。さらに、確保した各要素をグローバルカウンタ値で初期化する。

カウンタ動作:
- テスト開始時のカウンタ初期値は 1
- `alloc(size=N)` は要素 0..N-1 にカウンタ値 C, C+1, ..., C+N-1 を書き込む
- alloc 完了後、カウンタは C+N に進む

例: `alloc(3)` → `alloc(2)` の場合
- 1 回目: 要素に [1, 2, 3] を書き込み、カウンタ → 4
- 2 回目: 要素に [4, 5] を書き込み、カウンタ → 6

WS 変換:
```
; メモリ確保
push <size>
call __rt_alloc        ; ptr がスタックトップに
push <var_heap_addr>
swap
store                  ; heap[var_addr] = ptr

; 要素初期化 (i = 0..size-1 に展開)
push <var_heap_addr>
retrieve               ; ptr
push <i>
add                    ; ptr + i
push <counter_addr>
retrieve               ; counter_val
push <i>
add                    ; counter_val + i
store                  ; heap[ptr + i] = counter_val + i

; カウンタ更新
push <counter_addr>
push <counter_addr>
retrieve
push <size>
add
store                  ; counter += size
```

> カウンタはヒープ上の予約アドレスに格納され、ランタイムでインクリメントされる。これによりループ内の `alloc` でも毎回異なる値が割り当てられる。

#### `free` — メモリ解放

```json
{ "op": "free", "var": "p1" }
```

`__rt_free(heap[var_addr])` を呼び出す。

WS 変換:
```
push <var_heap_addr>
retrieve               ; ptr = heap[var_addr]
call __rt_free
```

#### `load_print` — 確保領域の全要素を出力

```json
{ "op": "load_print", "var": "p1" }
```

変数 `var` に対応する確保領域のすべての要素を順に出力する。各値は数値として出力し、改行を付加する。

ミニコンパイラはテスト仕様を順に処理し、各変数の最後の `alloc` サイズを記録する。`load_print` はそのサイズ分の要素を出力するコードを生成する。

WS 変換 (size=N の場合、i=0..N-1 に展開):
```
push <var_heap_addr>
retrieve               ; ptr
push <i>
add                    ; ptr + i
retrieve               ; value = heap[ptr + i]
output_num             ; print value
push 10
output_char            ; print '\n'
```

#### `print` — 即値を出力

```json
{ "op": "print", "value": 99 }
```

WS 変換:
```
push <value>
output_num
push 10
output_char            ; print '\n'
```

#### `assert_var_ne` — 2 変数の値が異なることを検証

```json
{ "op": "assert_var_ne", "var1": "p1", "var2": "p2" }
```

`heap[var1_addr] == heap[var2_addr]` ならランタイムエラーで異常終了する。

WS 変換:
```
push <var1_heap_addr>
retrieve
push <var2_heap_addr>
retrieve
sub                    ; diff = var1 - var2
jz __test_fail         ; 0 ならば失敗
```

#### `heap_print` — ヒープアドレスの値を直接出力

```json
{ "op": "heap_print", "address": 5 }
```

`print(heap[address])` を実行する。アロケータの内部メタデータ検証に使用。

WS 変換:
```
push <address>
retrieve
output_num
push 10
output_char
```

#### `loop` — 反復実行

```json
{
  "op": "loop",
  "count": 10,
  "body": [
    { "op": "alloc", "var": "tmp", "size": 4 },
    { "op": "free", "var": "tmp" }
  ]
}
```

`body` を `count` 回繰り返す。カウンタはランタイムで管理されるため、ループ内の `alloc` でも毎回異なる値で初期化される。

### 検証方法 (Check)

#### `alloc_io` — 標準出力の完全一致検証

```json
{
  "type": "alloc_io",
  "stdout": "1\n2\n3\n"
}
```

テスト実行後の stdout が期待値と完全一致することを検証する。カウンタベースの初期化により出力は決定的であるため、完全一致で十分。

#### `alloc_runtime_error` — ランタイムエラーの検証

```json
{
  "type": "alloc_runtime_error",
  "error": "AssertionFailed"
}
```

テスト実行がランタイムエラーで終了することを検証する（`assert_var_ne` の失敗等）。

## 変数ストレージとカウンタ

### 変数 → ヒープアドレスのマッピング

テスト内の変数とカウンタはヒープの予約領域に格納する。

```
ヒープレイアウト (GLOBAL_PTR 起点):

  heap[GLOBAL_PTR + 0]                          ← カウンタ (初期値 1)
  heap[GLOBAL_PTR + 1 .. + var_count]            ← 変数ストレージ
  heap[GLOBAL_PTR + 1 + var_count .. ]           ← FSBA テーブル以降

例:
  config.global_heap_size = 0
  vars = ["p1", "p2"]

  counter → heap[8]   (GLOBAL_PTR + 0)
  p1      → heap[9]   (GLOBAL_PTR + 1)
  p2      → heap[10]  (GLOBAL_PTR + 2)

  実効 global_heap_size = 1(カウンタ) + 2(変数) = 3
```

変数は `vars` 配列の宣言順に、カウンタの直後に配置する。アドレス計算はコンパイル時に決定的。

## ディレクトリ構成

### 新規追加

```
resources/
  tests_alloc/                    ← 新規ディレクトリ
    README.md                     ← テスト形式の説明
    test-manifest.yaml            ← テスト一覧 (build.rs で使用)
    basic/                        ← 基本的な alloc/free テスト
    fsba/                         ← FSBA 固有テスト
    stress/                       ← ストレステスト
    metadata/                     ← メタデータ・内部状態検証

src/
  compiler_ws/
    alloc_runtime.rs              ← 新規: アロケータランタイム生成（分離可能）

tests/
  alloc_test.rs                   ← 新規: 分離テストランナー
```

### build.rs の拡張

`resources/tests_alloc/test-manifest.yaml` からテスト関数を自動生成する `generate_alloc_tests()` を追加。

## ミニコンパイラの設計

### 概要

テスト用ミニコンパイラは `tests/alloc_test.rs` 内に実装する。

処理フロー:
1. JSON テストファイルをパース
2. 変数→アドレス、カウンタアドレスのマッピング構築
3. アロケータ初期化コード生成（`alloc_runtime` を呼び出す）+ カウンタ初期化
4. `steps` をイテレートし、各操作を WS 命令列に変換
5. Exit 命令 + サブルーチン定義 + テスト失敗ハンドラを結合

テスト専用ロジックであり、`tests/` ディレクトリ内で閉じる。

### WS コード生成の全体構造

```rust
fn compile_alloc_test(spec: &AllocTestSpec) -> Vec<WhitespaceInstruction> {
    let mut instructions = Vec::new();
    let var_count = spec.vars.len() as i64;

    // 1. アロケータ初期化
    let effective_global_size = 1 /* counter */ + var_count + spec.config.global_heap_size;
    instructions.extend(generate_allocator_header(effective_global_size));

    // 2. カウンタ初期化 (heap[counter_addr] = 1)
    let counter_addr = GLOBAL_PTR + spec.config.global_heap_size;
    instructions.push(Push(counter_addr));
    instructions.push(Push(1));
    instructions.push(Store);

    // 3. テスト操作を WS 命令に変換
    let var_map = build_var_map(&spec.vars, counter_addr + 1);
    let mut alloc_sizes: HashMap<String, i64> = HashMap::new();
    for step in &spec.steps {
        instructions.extend(compile_step(step, &var_map, counter_addr, &mut alloc_sizes));
    }

    // 4. Exit + サブルーチン + 失敗ハンドラ
    instructions.push(Exit);
    instructions.extend(generate_allocator_subroutines());
    instructions.extend(generate_test_fail_handler());

    instructions
}
```

### alloc_runtime モジュールの公開 API

```rust
/// アロケータの初期化コード（ヘッダーに埋め込み）
pub fn generate_allocator_header(global_heap_size: i64) -> Vec<WhitespaceInstruction>;

/// アロケータのサブルーチン定義 (__rt_alloc, __rt_free 等)
pub fn generate_allocator_subroutines() -> Vec<WhitespaceInstruction>;
```

## テストケース例

### alloc_basic_001: 1 セル確保・読み取り

```json
{
  "description": "alloc(1) で 1 セル確保、自動初期化値を読み取り",
  "vars": ["p1"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 1 },
    { "op": "load_print", "var": "p1" },
    { "op": "free", "var": "p1" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "1\n"
  }
}
```

> カウンタ 1 から開始、alloc(1) で要素 [1]、カウンタ → 2

### alloc_multi_001: 複数ブロック確保・非重複・データ非干渉

```json
{
  "description": "複数回 alloc で異なるブロック確保、データ非干渉を検証",
  "vars": ["p1", "p2", "p3"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 2 },
    { "op": "alloc", "var": "p2", "size": 2 },
    { "op": "alloc", "var": "p3", "size": 2 },
    { "op": "assert_var_ne", "var1": "p1", "var2": "p2" },
    { "op": "assert_var_ne", "var1": "p2", "var2": "p3" },
    { "op": "assert_var_ne", "var1": "p1", "var2": "p3" },
    { "op": "load_print", "var": "p1" },
    { "op": "load_print", "var": "p2" },
    { "op": "load_print", "var": "p3" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "1\n2\n3\n4\n5\n6\n"
  }
}
```

> p1=[1,2], p2=[3,4], p3=[5,6]。alloc 間でデータが干渉していないことを出力値で検証。

### alloc_free_reuse_001: 解放後の再確保

```json
{
  "description": "alloc → free → alloc で再確保、新しい値で正常に初期化",
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 2 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 2 },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "3\n4\n"
  }
}
```

> p1=[1,2] (カウンタ→3), free(p1), p2=[3,4]。再確保されたブロックが正しく書き込み可能なことを検証。

### fsba_large_fallback_001: FSBA → 汎用フォールバック

```json
{
  "description": "32 セル超の alloc が汎用アロケータにフォールバック",
  "vars": ["p_small", "p_large"],
  "steps": [
    { "op": "alloc", "var": "p_small", "size": 3 },
    { "op": "alloc", "var": "p_large", "size": 50 },
    { "op": "assert_var_ne", "var1": "p_small", "var2": "p_large" },
    { "op": "load_print", "var": "p_small" },
    { "op": "free", "var": "p_small" },
    { "op": "free", "var": "p_large" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "1\n2\n3\n"
  }
}
```

### repeated_alloc_free_001: 繰り返し alloc/free ストレステスト

```json
{
  "description": "同サイズの alloc/free を 100 回繰り返し、ヒープが破壊されないことを確認",
  "config": {
    "max_steps": 1000000
  },
  "vars": ["p", "sentinel"],
  "steps": [
    { "op": "alloc", "var": "sentinel", "size": 1 },
    {
      "op": "loop",
      "count": 100,
      "body": [
        { "op": "alloc", "var": "p", "size": 4 },
        { "op": "free", "var": "p" }
      ]
    },
    { "op": "load_print", "var": "sentinel" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "1\n"
  }
}
```

> sentinel=[1] はループ前に確保。100 回の alloc/free でヒープが破壊されなければ sentinel の値は 1 のまま。

## 計画されるテスト一覧

| テスト名 | カテゴリ | 検証内容 |
|---|---|---|
| `alloc_basic_001` | basic | `alloc(1)` 確保・読み取り |
| `alloc_basic_002` | basic | `alloc(10)` 全要素読み取り |
| `alloc_multi_001` | basic | 複数 `alloc` で非重複・データ非干渉 |
| `alloc_free_reuse_001` | basic | `alloc` → `free` → `alloc` で再確保 |
| `alloc_free_reuse_002` | basic | 異なるサイズの確保・解放・再確保 |
| `alloc_split_001` | basic | 大きなフリーブロックの分割 |
| `alloc_zero_size_001` | basic | `alloc(0)` → 最小ブロック確保 |
| `fsba_class_reuse_001` | fsba | 同一サイズクラスの alloc/free/alloc 再利用 |
| `fsba_different_class_001` | fsba | 異なるサイズクラスの独立管理 |
| `fsba_roundup_001` | fsba | サイズ切り上げの正しさ |
| `fsba_large_fallback_001` | fsba | 32 セル超の汎用フォールバック |
| `repeated_alloc_free_001` | stress | 100 回の alloc/free 繰り返し |
| `many_small_allocs_001` | stress | 多数の小ブロック確保 |
| `heap_top_growth_001` | metadata | ALLOC_HEAP_TOP の成長を `heap_print` で検証 |
| `free_list_structure_001` | metadata | フリーリストの構造を `heap_print` で検証 |

## テスト実行

```bash
cargo test --test alloc_test              # 分離テストのみ
cargo test --test alloc_test -- alloc_basic_001  # 特定テスト
cargo test --test alloc_test -- fsba      # FSBA テストのみ
```

## alloc_runtime モジュールの設計指針

1. **CodeGenContext に依存しない**: 公開関数は `global_heap_size` 等を直接引数に取る
2. **入出力は `Vec<WhitespaceInstruction>` のみ**
3. **ラベル名は予約プレフィックスを使用**: `__rt_alloc`, `__rt_free` 等

テストランナーからも `compiler_ws` パイプラインからも呼び出し可能。`alloc_runtime.rs` 内の `#[cfg(test)] mod tests` はラベル生成等の unit テスト、`tests/alloc_test.rs` は WS VM 上の統合テスト。
