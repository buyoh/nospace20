# メモリアロケータの分離テスト設計

## 目的

メモリアロケータの実装は複雑であるため、nospace コンパイラパイプライン全体（構文解析・意味解析・インタプリタ）から分離して、アロケータ単体でテスト可能な仕組みを構築する。

## 設計方針

### 分離テストの原則

1. **nospace パイプラインに非依存**: token_parser, tree_parser, semantic_analyzer, interpreter に一切依存しない
2. **依存する最小構成**:
   - `compiler_ws/alloc_runtime` — アロケータの WS コード生成（テスト対象）
   - `whitespace/interpreter` — WS VM（テスト実行環境）
   - テスト用ミニコンパイラ — JSON テスト仕様 → WS 命令列の変換
3. **JSON ベースのテスト記述**: アロケータ操作と期待結果を JSON で宣言的に記述

### アーキテクチャ

```
┌──────────────────────────────────────────────────┐
│                   テスト実行                       │
│                                                   │
│  ┌──────────┐   ┌────────────────┐               │
│  │ JSON     │   │ alloc_runtime  │               │
│  │ テスト   │──→│ (コード生成)    │               │
│  │ 仕様     │   └────────┬───────┘               │
│  └─────┬────┘            │                       │
│        │            WS サブルーチン               │
│        │                 │                       │
│  ┌─────▼────┐            │                       │
│  │ ミニ     │            │                       │
│  │ コンパイラ│────────────┤                       │
│  └─────┬────┘            │                       │
│        │           ┌─────▼─────┐                 │
│        │           │  結合     │                 │
│        │           │ WS コード │                 │
│        └──────────→│ (init +   │                 │
│           テスト    │  runtime +│                 │
│           操作 WS   │  test ops)│                 │
│                    └─────┬─────┘                 │
│                          │                       │
│                    ┌─────▼─────┐                 │
│                    │WhitespaceVM│                │
│                    │ (実行)     │                │
│                    └─────┬─────┘                 │
│                          │                       │
│                    stdout / エラー                │
│                          │                       │
│                    ┌─────▼─────┐                 │
│                    │ 結果検証   │                 │
│                    └───────────┘                 │
└──────────────────────────────────────────────────┘
```

**依存関係（テスト実行時）**:

```
tests/alloc_test.rs
  ├── nospace20::compiler_ws::alloc_runtime  ← テスト対象
  └── nospace20::whitespace::WhitespaceVM    ← 実行環境

resources/tests_alloc/*.test.json           ← テストデータ
```

**依存しないもの**:
- `src/token_parser/` (字句解析)
- `src/tree_parser/` (構文解析)
- `src/semantic_analyzer/` (意味解析)
- `src/interpreter/` (インタプリタ)
- `src/compiler_ws/` の alloc_runtime 以外のモジュール（statement, expression 等）

## JSON テスト仕様フォーマット

### 基本構造

```json
{
  "description": "テストの説明",
  "config": {
    "global_heap_size": 0,
    "max_steps": 100000
  },
  "vars": ["p1", "p2", "arr"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 3 },
    { "op": "store", "var": "p1", "offset": 0, "value": 42 },
    { "op": "load_print", "var": "p1", "offset": 0 },
    { "op": "free", "var": "p1" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "42\n"
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

#### `alloc` — メモリ確保

```json
{ "op": "alloc", "var": "p1", "size": 3 }
```

`__rt_alloc(size)` を呼び出し、返されたポインタを変数 `var` に格納する。

WS 変換:
```
push <size>
call __rt_alloc        ; ptr がスタックトップに
push <var_heap_addr>
swap
store                  ; heap[var_addr] = ptr
```

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

#### `store` — 確保領域への書き込み

```json
{ "op": "store", "var": "p1", "offset": 0, "value": 42 }
```

`heap[heap[var_addr] + offset] = value` を実行する。

WS 変換:
```
push <var_heap_addr>
retrieve               ; ptr
push <offset>
add                    ; ptr + offset
push <value>
store                  ; heap[ptr + offset] = value
```

#### `store_var` — 変数の値を確保領域に書き込み

```json
{ "op": "store_var", "var": "p1", "offset": 0, "src_var": "p2" }
```

`heap[heap[var1_addr] + offset] = heap[var2_addr]` を実行する。ポインタの連結リスト構築等に使用。

WS 変換:
```
push <var_heap_addr>
retrieve               ; ptr1
push <offset>
add                    ; ptr1 + offset
push <src_var_heap_addr>
retrieve               ; value = heap[src_var_addr]
store                  ; heap[ptr1 + offset] = value
```

#### `load_print` — 確保領域から読み込んで出力

```json
{ "op": "load_print", "var": "p1", "offset": 0 }
```

`print(heap[heap[var_addr] + offset])` を実行する。数値として出力し、改行を付加する。

WS 変換:
```
push <var_heap_addr>
retrieve               ; ptr
push <offset>
add                    ; ptr + offset
retrieve               ; value = heap[ptr + offset]
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

#### `print_var` — 変数の値を出力

```json
{ "op": "print_var", "var": "p1" }
```

変数に格納されたポインタ値そのものを出力する（デバッグ用）。

WS 変換:
```
push <var_heap_addr>
retrieve               ; value = heap[var_addr]
output_num
push 10
output_char
```

#### `heap_print` — ヒープアドレスの値を直接出力

```json
{ "op": "heap_print", "address": 5 }
```

`print(heap[address])` を実行する。メタデータの検証に使用。

WS 変換:
```
push <address>
retrieve
output_num
push 10
output_char
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

#### `loop` — 反復実行

```json
{
  "op": "loop",
  "count": 10,
  "counter_var": "i",
  "body": [
    { "op": "alloc", "var": "tmp", "size": 4 },
    { "op": "free", "var": "tmp" }
  ]
}
```

`body` を `count` 回繰り返す。`counter_var` を指定した場合、そのループ変数が参照可能。
大量の alloc/free 繰り返しテストに使用する。

### 検証方法 (Check)

#### `alloc_io` — 標準出力の検証

```json
{
  "type": "alloc_io",
  "stdout": "42\n99\n"
}
```

テスト実行後の stdout が期待値と一致することを検証する。

#### `alloc_runtime_error` — ランタイムエラーの検証

```json
{
  "type": "alloc_runtime_error",
  "error": "AssertionFailed"
}
```

テスト実行がランタイムエラーで終了することを検証する（`assert_var_ne` の失敗等）。

## 変数ストレージ

### 変数 → ヒープアドレスのマッピング

テスト内の変数はヒープの予約領域に格納する。テスト用ミニコンパイラがビルド時に変数名→アドレスのマッピングを生成する。

```
変数ストレージ: heap[GLOBAL_PTR + global_heap_size .. GLOBAL_PTR + global_heap_size + var_count - 1]

例:
  config.global_heap_size = 0
  vars = ["p1", "p2", "arr"]

  p1  → heap[8]   (GLOBAL_PTR + 0)
  p2  → heap[9]   (GLOBAL_PTR + 1)
  arr → heap[10]   (GLOBAL_PTR + 2)

  実効 global_heap_size = 0 + 3 = 3
  FSBA テーブル = heap[11..15]
  マネージドヒープ開始 = heap[16]
```

変数は `vars` 配列の宣言順に、グローバル変数領域の末尾に配置する。これにより:
- `config.global_heap_size` で指定されたユーザーグローバル領域と衝突しない
- アロケータが管理するマネージドヒープとも衝突しない
- アドレス計算がコンパイル時に決定的

## ディレクトリ構成

### 新規追加

```
resources/
  tests_alloc/                    ← 新規ディレクトリ
    README.md                     ← テスト形式の説明
    test-manifest.yaml            ← テスト一覧 (build.rs で使用)
    basic/                        ← 基本的な alloc/free テスト
      alloc_basic_001.test.json
      alloc_basic_002.test.json
      alloc_multi_001.test.json
      alloc_free_reuse_001.test.json
      alloc_free_reuse_002.test.json
      alloc_split_001.test.json
      alloc_zero_size_001.test.json
    fsba/                         ← FSBA 固有テスト
      fsba_class_reuse_001.test.json
      fsba_different_class_001.test.json
      fsba_roundup_001.test.json
      fsba_large_fallback_001.test.json
    stress/                       ← ストレステスト
      repeated_alloc_free_001.test.json
      many_small_allocs_001.test.json
    metadata/                     ← メタデータ・内部状態検証
      heap_top_growth_001.test.json
      free_list_structure_001.test.json
```

### ソースコード

```
src/
  compiler_ws/
    alloc_runtime.rs              ← 新規: アロケータランタイム生成（分離可能）
    mod.rs                        ← alloc_runtime を公開

tests/
  alloc_test.rs                   ← 新規: 分離テストランナー
```

### build.rs の拡張

```
build.rs
  └── generate_alloc_tests()      ← 新規: resources/tests_alloc/ からテスト関数生成
```

## ミニコンパイラの設計

### 概要

テスト用ミニコンパイラは以下を行う Rust モジュール:

1. JSON テストファイルをパース
2. 変数→アドレスのマッピング構築
3. アロケータ初期化コード生成（`alloc_runtime` を呼び出す）
4. `steps` をイテレートし、各操作を WS 命令列に変換
5. Exit 命令を追加
6. 初期化コード + テスト操作コード + ランタイムサブルーチンを結合

### 実装場所

テストランナー (`tests/alloc_test.rs`) 内に実装する。ライブラリコードとしてではなく、テストコードとして配置する。

理由:
- テスト専用の変換ロジックであり、本体ライブラリに含める必要がない
- `tests/` ディレクトリは `nospace20` クレートを依存として使えるため、`alloc_runtime` に直接アクセス可能
- テスト追加時にミニコンパイラの変更が必要になっても、テストコード内で閉じる

### WS コード生成の全体構造

```rust
fn compile_alloc_test(spec: &AllocTestSpec) -> Vec<WhitespaceInstruction> {
    let mut instructions = Vec::new();

    // 1. アロケータ初期化コードを生成
    //    (alloc_runtime からヘッダー部分を取得)
    let effective_global_size = spec.config.global_heap_size + spec.vars.len() as i64;
    instructions.extend(generate_allocator_header(effective_global_size));

    // 2. テスト操作を WS 命令に変換
    let var_map = build_var_map(&spec.vars, spec.config.global_heap_size);
    for step in &spec.steps {
        instructions.extend(compile_step(step, &var_map));
    }

    // 3. Exit 命令
    instructions.push(WhitespaceInstruction::Exit);

    // 4. アロケータサブルーチン定義を追加
    //    (alloc_runtime からサブルーチン部分を取得)
    instructions.extend(generate_allocator_subroutines());

    // 5. テスト失敗ハンドラ
    instructions.extend(generate_test_fail_handler());

    instructions
}
```

### alloc_runtime モジュールの公開 API

`compiler_ws/alloc_runtime.rs` は以下の公開関数を提供する:

```rust
/// アロケータの初期化コード（ヘッダーに埋め込み）
/// global_heap_size: グローバル変数領域 + テスト変数領域のサイズ
pub fn generate_allocator_header(global_heap_size: i64) -> Vec<WhitespaceInstruction> { ... }

/// アロケータのサブルーチン定義
/// __rt_alloc, __rt_free 等のラベル定義と実装
pub fn generate_allocator_subroutines() -> Vec<WhitespaceInstruction> { ... }
```

この API により、テストランナーは nospace コンパイラの他のモジュールに依存せず、アロケータランタイムだけをテスト用 WS コードに組み込める。

## テストマニフェスト

### test-manifest.yaml 形式

```yaml
tests:
  # 基本テスト
  - name: alloc_basic_001
    type: alloc_io
    path: basic/alloc_basic_001
    comment: "alloc(1) で 1 セル確保、書き込み・読み取り"

  - name: alloc_basic_002
    type: alloc_io
    path: basic/alloc_basic_002
    comment: "alloc(10) で 10 セル確保、全セル書き込み・読み取り"

  # FSBA テスト
  - name: fsba_class_reuse_001
    type: alloc_io
    path: fsba/fsba_class_reuse_001
    comment: "FSBA: 同一サイズクラスの alloc/free/alloc で再利用"

  # ランタイムエラーテスト
  - name: assert_overlap_fail_001
    type: alloc_runtime_error
    path: basic/assert_overlap_fail_001
    comment: "同じポインタが返された場合に assert_var_ne で失敗"
```

### build.rs 統合

`build.rs` に `generate_alloc_tests()` 関数を追加:

```rust
fn generate_alloc_tests() {
    let manifest_path = "resources/tests_alloc/test-manifest.yaml";
    if !Path::new(manifest_path).exists() {
        return;
    }

    println!("cargo:rerun-if-changed=resources/tests_alloc/test-manifest.yaml");

    let manifest: TestManifest = /* YAML 読み込み */;
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_alloc_tests.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    for test in manifest.tests {
        match test.test_type.as_str() {
            "alloc_io" => {
                writeln!(f, r#"
                    #[test]
                    fn {name}() {{
                        test_alloc_io_base("{path}")
                    }}
                "#, name = test.name, path = test.path).unwrap();
            }
            "alloc_runtime_error" => {
                writeln!(f, r#"
                    #[test]
                    fn {name}() {{
                        test_alloc_runtime_error_base("{path}")
                    }}
                "#, name = test.name, path = test.path).unwrap();
            }
            _ => panic!("Unknown alloc test type"),
        }
    }
}
```

`tests/alloc_test.rs` は生成されたテスト関数を include:

```rust
include!(concat!(env!("OUT_DIR"), "/generated_alloc_tests.rs"));
```

## テストケース例

### alloc_basic_001: 1 セル確保・書き込み・読み取り

```json
{
  "description": "alloc(1) で 1 セル確保、書き込み・読み取り",
  "vars": ["p1"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 1 },
    { "op": "store", "var": "p1", "offset": 0, "value": 42 },
    { "op": "load_print", "var": "p1", "offset": 0 },
    { "op": "free", "var": "p1" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "42\n"
  }
}
```

### alloc_multi_001: 複数ブロック確保・非重複検証

```json
{
  "description": "複数回 alloc で異なるブロック確保、データ非干渉を検証",
  "vars": ["p1", "p2", "p3"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 3 },
    { "op": "alloc", "var": "p2", "size": 3 },
    { "op": "alloc", "var": "p3", "size": 3 },
    { "op": "assert_var_ne", "var1": "p1", "var2": "p2" },
    { "op": "assert_var_ne", "var1": "p2", "var2": "p3" },
    { "op": "assert_var_ne", "var1": "p1", "var2": "p3" },
    { "op": "store", "var": "p1", "offset": 0, "value": 100 },
    { "op": "store", "var": "p2", "offset": 0, "value": 200 },
    { "op": "store", "var": "p3", "offset": 0, "value": 300 },
    { "op": "store", "var": "p1", "offset": 1, "value": 101 },
    { "op": "store", "var": "p2", "offset": 1, "value": 201 },
    { "op": "store", "var": "p3", "offset": 1, "value": 301 },
    { "op": "load_print", "var": "p1", "offset": 0 },
    { "op": "load_print", "var": "p1", "offset": 1 },
    { "op": "load_print", "var": "p2", "offset": 0 },
    { "op": "load_print", "var": "p2", "offset": 1 },
    { "op": "load_print", "var": "p3", "offset": 0 },
    { "op": "load_print", "var": "p3", "offset": 1 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "100\n101\n200\n201\n300\n301\n"
  }
}
```

### alloc_free_reuse_001: 解放ブロック再利用

```json
{
  "description": "alloc → free → alloc でブロック再利用、データ書き込み確認",
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 4 },
    { "op": "print_var", "var": "p1" },
    { "op": "store", "var": "p1", "offset": 0, "value": 111 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 4 },
    { "op": "print_var", "var": "p2" },
    { "op": "store", "var": "p2", "offset": 0, "value": 222 },
    { "op": "load_print", "var": "p2", "offset": 0 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout_check": "last_line",
    "stdout_contains": "222"
  }
}
```

> 注: ポインタ再利用の検証は、`p1` と `p2` のアドレスが等しいことを直接検証するのが理想だが、アドレスの具体値はグローバルヒープサイズに依存する。代替として `stdout_contains` 検証を使用するか、特殊なアサーション操作を追加する。

### fsba_class_reuse_001: FSBA 同一クラス再利用

```json
{
  "description": "FSBA: サイズクラス 2 (alloc(1)) の alloc/free/alloc で再利用",
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 1 },
    { "op": "print_var", "var": "p1" },
    { "op": "store", "var": "p1", "offset": 0, "value": 42 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 1 },
    { "op": "print_var", "var": "p2" },
    { "op": "store", "var": "p2", "offset": 0, "value": 99 },
    { "op": "load_print", "var": "p2", "offset": 0 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout_check": "last_line",
    "stdout_contains": "99"
  }
}
```

### fsba_large_fallback_001: FSBA → 汎用フォールバック

```json
{
  "description": "32 セル超の alloc が汎用アロケータにフォールバック",
  "vars": ["p_small", "p_large"],
  "steps": [
    { "op": "alloc", "var": "p_small", "size": 3 },
    { "op": "alloc", "var": "p_large", "size": 50 },
    { "op": "assert_var_ne", "var1": "p_small", "var2": "p_large" },
    { "op": "store", "var": "p_small", "offset": 0, "value": 10 },
    { "op": "store", "var": "p_large", "offset": 0, "value": 20 },
    { "op": "store", "var": "p_large", "offset": 49, "value": 99 },
    { "op": "load_print", "var": "p_small", "offset": 0 },
    { "op": "load_print", "var": "p_large", "offset": 0 },
    { "op": "load_print", "var": "p_large", "offset": 49 },
    { "op": "free", "var": "p_small" },
    { "op": "free", "var": "p_large" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "10\n20\n99\n"
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
    { "op": "store", "var": "sentinel", "offset": 0, "value": 9999 },
    {
      "op": "loop",
      "count": 100,
      "body": [
        { "op": "alloc", "var": "p", "size": 4 },
        { "op": "store", "var": "p", "offset": 0, "value": 42 },
        { "op": "free", "var": "p" }
      ]
    },
    { "op": "load_print", "var": "sentinel", "offset": 0 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "9999\n"
  }
}
```

## 計画されるテスト一覧

### Phase 2: アロケータ基盤テスト

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
| `free_list_structure_001` | metadata | フリーリストの構造を heap_print で検証 |

## テスト実行

```bash
# 分離テストのみ実行
cargo test --test alloc_test

# 特定のテストを実行
cargo test --test alloc_test -- alloc_basic_001

# FSBA テストのみ
cargo test --test alloc_test -- fsba
```

## alloc_runtime モジュールの設計指針

### 分離可能性

`alloc_runtime.rs` は `compiler_ws/` 内に配置するが、以下の設計制約に従う:

1. **CodeGenContext に依存しない**: alloc_runtime の公開関数は `CodeGenContext` を引数に取らない。必要なパラメータ（`global_heap_size` 等）は直接渡す
2. **入出力は `Vec<WhitespaceInstruction>` のみ**: WS 命令列の生成に特化する
3. **ラベル名は予約プレフィックスを使用**: `__rt_alloc`, `__rt_free`, `__rt_fsba_*` 等、ユーザーコードと衝突しないラベル

このにより、テストランナーから直接呼び出し可能で、かつ `compiler_ws` のコンパイルパイプライン（`builtin.rs` の `generate_header` 等）からも呼び出し可能となる。

### 内部テストとの関係

`alloc_runtime.rs` 内の `#[cfg(test)] mod tests` では、ラベル生成やメタデータ設定の unit テストを行う。これとは別に `tests/alloc_test.rs` の分離テストは、生成された WS コード全体を WhitespaceVM 上で実行する統合レベルのテストとなる。
