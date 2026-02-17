# Whitespace コンパイラでの static 変数サポート

作成日: 2026-02-17  
更新日: 2026-02-18  
ステータス: ✅ 実装完了（一部制限あり）

## 実装完了サマリー

**日時**: 2026-02-18  
**実装内容**: static 変数のグローバルヒープ配置と初期化コード生成

### 実装した変更

1. **`src/compiler_ws/context.rs`**
   - `static_var_global_offsets: HashMap<(usize, usize), i64>` フィールド追加
   - `static_var_total_size: i64` フィールド追加
   - `current_func_index: Option<usize>` フィールド追加
   - `current_func_scope: Option<&'a Scope>` フィールド追加
   - `compute_static_var_offsets()` 関数追加
   - `enter_function()` に関数インデックスと関数スコープ引数を追加
   - `enter_function_for_static_init()` 関数追加
   - `get_var_info()` での static 変数判定ロジック追加
   - `global_heap_size()` に static 変数領域サイズを加算

2. **`src/compiler_ws/statement.rs`**
   - `generate_scope()` に関数内 static 変数の初期化コード生成を追加
   - `generate_function_definition()` に `func_index` 引数を追加

### テスト結果

✅ **成功**: `test_scope_scope_static_init_value_persist_001_ws_self`  
   - 関数内 static 変数の値が呼び出しをまたいで永続化されることを確認

⚠️ **新たな失敗**: `test_scope_scope_static_mixed_001_ws_self`  
   - ネストされた関数から親関数の static 変数にアクセスする際に失敗
   - 調査ドキュメント: [whitespace-static-nested-function-issue.md](./whitespace-static-nested-function-issue.md)

### 制限事項

現在の実装では、ルートレベルの関数の static 変数のみサポート。  
ネストされた関数（関数内で定義された関数）の static 変数や、ネストされた関数から親関数の static 変数へのアクセスは未対応。

## 問題の概要

`test_scope_scope_static_init_value_persist_001_ws_self` テストが失敗している。
関数内の `static:` 変数の初期値が、Whitespace コンパイラで正しく処理されない。

Interpreter では正しく動作するが、Whitespace コンパイラでは3つの欠陥により失敗する。

## テストケース

`resources/tests/passes/scope/scope_static_init_value_persist_001.ns`

```nospace
func: counter() {
  static: count(100);
  count = count + 1;
  return: count;
}

func: main() {
  __assert(counter() == 101);
  __assert(counter() == 102);
  __assert(counter() == 103);
  __trace(0);
}
```

## 根本原因分析

### 欠陥 1: 関数内 static 変数の初期化式が未出力

`generate_function_definition()` は `func.block.statements` のみを処理し、
`func.block.scope.static_init_statements` を**無視**している。
つまり `count = 100` という初期化コードが一切生成されない。

**該当箇所**: `src/compiler_ws/statement.rs` の `generate_function_definition()`

### 欠陥 2: static 変数がローカル変数として扱われる

セマンティックアナライザが生成する `IdentifierRef` は `is_global: false` であるため、
コンパイラは `count` をローカルスタックフレーム上に配置する。
スタックフレームは関数呼び出しごとに新規作成されるため、値は**永続化されない**。

**該当箇所**: `src/compiler_ws/context.rs` の `get_var_info()`

### 欠陥 3: 値の永続化メカニズムが存在しない

インタプリタの `function_static_storage` に相当する仕組みがコンパイラに存在しない。

### テスト失敗の流れ

```
counter() 1回目:
  count は未初期化（ヒープのデフォルト値 0）
  count = 0 + 1 = 1
  return 1
  → __assert(1 == 101) → 失敗（AssertionFailed）
```

## 設計方針

### アプローチ: static 変数をグローバルヒープ領域に配置

Whitespace のメモリモデルでは、ローカル変数はスタックフレーム上にあり関数呼び出しごとに作成・破棄される。
一方、グローバル変数領域（`GLOBAL_PTR` 以降、`LOCAL_HEAP_BEGIN` 未満）は永続的である。

**static 変数をグローバルヒープ領域に配置し、初期化を main 呼び出し前に1回だけ実行する。**

これにより:
- static 変数は関数の呼び出しをまたいで値が保持される
- 初期化式は `generate_scope()` のルートレベルで 1 回だけコード生成される
- 関数内での参照・代入はグローバル変数アクセスと同じ命令パターンになる

### Whitespace ヒープメモリレイアウト（変更後）

```
アドレス 0-1: 未使用
アドレス 2: LOCAL_HEAP_BEGIN ポインタ
アドレス 3: LOCAL_HEAP_END ポインタ
アドレス 4: TEMP_PTR
アドレス 5-7: 未使用
アドレス 8+:           グローバル変数領域（scope.variable_count スロット）
アドレス 8+G+:         関数 static 変数領域（各関数の static 変数を連続配置）  ← 新規
アドレス 8+G+S+:       ローカルヒープ（スタックフレーム）
```

- `G` = グローバル変数スロット数
- `S` = 全関数の static 変数スロット合計数

## 詳細設計

### ステップ 1: static 変数のグローバルオフセット計算

#### 変更モジュール: `src/compiler_ws/context.rs`

`CodeGenContext` に static 変数のグローバルオフセットマッピングを追加する。

```rust
/// 関数内 static 変数のグローバルオフセット
/// キー: (関数インデックス, ローカル変数スロットインデックス)
/// 値: グローバルヒープ上のオフセット（GLOBAL_PTR からの相対）
static_var_global_offsets: HashMap<(usize, usize), i64>,

/// static 変数領域の合計サイズ
static_var_total_size: i64,
```

初期化時に全関数をスキャンし、static 変数にグローバルオフセットを割り当てる:

```rust
fn compute_static_var_offsets(scope: &Scope) -> (HashMap<(usize, usize), i64>, i64) {
    let mut offsets = HashMap::new();
    let mut next_offset = scope.variable_count as i64; // グローバル変数の直後
    
    for (func_idx, func) in scope.functions.iter().enumerate() {
        for var in &func.block.scope.variables {
            if var.is_static {
                let slot_count = var.array_size.unwrap_or(1);
                offsets.insert((func_idx, var.slot_index), next_offset);
                next_offset += slot_count as i64;
            }
        }
    }
    
    let total_size = next_offset - scope.variable_count as i64;
    (offsets, total_size)
}
```

#### `global_heap_size()` の変更

static 変数領域を含めたサイズを返すように変更:

```rust
pub fn global_heap_size(&self) -> i64 {
    self.scope.variable_count as i64 + self.static_var_total_size
}
```

### ステップ 2: `get_var_info()` での static 変数の特別扱い

#### 変更モジュール: `src/compiler_ws/context.rs`

現在の `get_var_info()` は `IdentifierRef` だけを見ているが、
static 変数であるかを判別する追加情報が必要。

#### 方針: セマンティックアナライザは変更しない

コンパイラ側で関数コンテキスト内の変数参照時に、
セマンティックアナライザの `Variable.is_static` フラグを参照して判断する。

`enter_function()` で現在の関数インデックスを保持し、
`get_var_info()` で変数が static かどうかを確認する:

```rust
pub fn enter_function(
    &self,
    total_var_count: usize,
    func_scope_var_count: usize,
    func_index: usize,          // 追加
    func_scope: &'a Scope,      // 追加: 関数のスコープ（Variable 情報参照用）
) -> CodeGenContext<'a> {
    CodeGenContext {
        // ...既存フィールド...
        current_func_index: Some(func_index),
        current_func_scope: Some(func_scope),
        // ...
    }
}
```

`get_var_info()` を拡張:

```rust
pub fn get_var_info(&self, var_ref: &IdentifierRef) -> VarInfo {
    if var_ref.is_global {
        VarInfo { scope: VarScope::Global, offset: var_ref.local_index as i64 }
    } else {
        // static 変数チェック（関数スコープ直下の変数のみ）
        if let (Some(func_idx), Some(func_scope)) = 
            (self.current_func_index, self.current_func_scope) 
        {
            // scope_depth == 0 で関数スコープ直下の変数を参照している場合
            if var_ref.scope_depth == 0 || var_ref.scope_depth >= self.scope_offsets.len() {
                // var_ref.local_index はスロットインデックスなので、
                // 該当する Variable を探す
                if let Some(global_offset) = self.static_var_global_offsets
                    .get(&(func_idx, var_ref.local_index)) 
                {
                    return VarInfo {
                        scope: VarScope::Global,
                        offset: *global_offset,
                    };
                }
            }
        }
        
        // 通常のローカル変数処理（既存コード）
        let scope_offsets_len = self.scope_offsets.len();
        if var_ref.scope_depth >= scope_offsets_len {
            VarInfo { scope: VarScope::Global, offset: var_ref.local_index as i64 }
        } else {
            let scope_idx = scope_offsets_len - 1 - var_ref.scope_depth;
            let base_offset = self.scope_offsets[scope_idx];
            VarInfo { scope: VarScope::Local, offset: base_offset + var_ref.local_index as i64 }
        }
    }
}
```

### ステップ 3: static 変数の初期化コード生成

#### 変更モジュール: `src/compiler_ws/statement.rs`

`generate_scope()` で、各関数の `static_init_statements` を main 呼び出し前に処理する。

```rust
pub fn generate_scope(ctx: &mut CodeGenContext, scope: &Scope) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // ① ルートレベルの static 初期化（既存）
    for stmt in &scope.static_init_statements {
        prog.append(generate_statement(ctx, stmt)?);
    }

    // ② 関数内 static 変数の初期化（新規）
    for (func_idx, func) in scope.functions.iter().enumerate() {
        if !func.block.scope.static_init_statements.is_empty() {
            let func_scope_var_count = func.block.scope.variable_count;
            let total_var_count = calculate_total_variable_count(&func.block);
            let mut static_ctx = ctx.enter_function_for_static_init(
                total_var_count, func_scope_var_count, func_idx, &func.block.scope
            );
            for stmt in &func.block.scope.static_init_statements {
                prog.append(generate_statement(&mut static_ctx, stmt)?);
            }
            ctx.sync_labels_from(&static_ctx);
        }
    }

    // ③ グローバル変数の初期化（既存）
    for stmt in &scope.root_statements {
        prog.append(generate_statement(ctx, stmt)?);
    }

    // ④ 関数定義（既存）
    for (i, func_name) in scope.symbol_table.function_names.iter().enumerate() {
        let func = &scope.functions[i];
        prog.append(generate_function_definition(ctx, func_name, func, i)?);
    }

    Ok(prog)
}
```

#### `enter_function_for_static_init()` の実装

static 初期化では、変数への書き込みはグローバルヒープ上で行われるため、
ローカルヒープの allocate/deallocate は不要。
それ以外は `enter_function()` と同じ動作でアドレス解決を行う。

```rust
pub fn enter_function_for_static_init(
    &self,
    total_var_count: usize,
    func_scope_var_count: usize,
    func_index: usize,
    func_scope: &'a Scope,
) -> CodeGenContext<'a> {
    // enter_function と同等だが、static 変数のグローバルオフセットマッピングを引き継ぐ
    // local_heap_size は 0（実際にはフレームを確保しない）
    CodeGenContext {
        scope: self.scope,
        labels: self.labels.clone(),
        is_global: false,
        local_heap_size: 0, // フレーム不要
        variables: HashMap::new(),
        loop_labels: Vec::new(),
        debug_ext: self.debug_ext,
        scope_offsets: vec![0],
        next_var_offset: func_scope_var_count as i64,
        current_func_index: Some(func_index),
        current_func_scope: Some(func_scope),
        static_var_global_offsets: self.static_var_global_offsets.clone(),
        static_var_total_size: self.static_var_total_size,
    }
}
```

### ステップ 4: `generate_function_definition()` の変更

#### 変更モジュール: `src/compiler_ws/statement.rs`

`enter_function()` に関数インデックスと関数スコープを渡すように変更:

```rust
fn generate_function_definition(
    ctx: &mut CodeGenContext,
    func_name: &str,
    func: &crate::semantic_analyzer::Function,
    func_index: usize,  // 追加
) -> Result<WsProgram, CompileError> {
    // ...
    let mut local_ctx = ctx.enter_function(
        total_var_count, func_scope_var_count,
        func_index, &func.block.scope,  // 追加
    );
    // ...残りは既存コードと同じ
}
```

`generate_scope()` の関数定義ループも修正:

```rust
for (i, func_name) in scope.symbol_table.function_names.iter().enumerate() {
    let func = &scope.functions[i];
    prog.append(generate_function_definition(ctx, func_name, func, i)?);  // i を追加
}
```

### ステップ 5: static 変数のローカルヒープサイズ補正

static 変数はグローバルヒープに配置するため、ローカルヒープサイズの計算から除く必要はない。
理由: `variable_count` はスロット数としてブロック内で一意に割り当てられており、
ローカル変数 + static 変数の合計がフレームサイズになっている。
static 変数のスロットは使われない「穴」になるが、正常動作に支障はない。

将来的な最適化として static 変数分のスロットを除外することは可能だが、
本タスクの対象外とする。

## 変更ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `src/compiler_ws/context.rs` | `static_var_global_offsets`, `static_var_total_size`, `current_func_index`, `current_func_scope` フィールド追加。`enter_function()` 引数追加。`enter_function_for_static_init()` 新規追加。`get_var_info()` static 変数判定追加。`global_heap_size()` static 変数領域加算。 |
| `src/compiler_ws/statement.rs` | `generate_scope()` に関数内 static 初期化コード生成を追加。`generate_function_definition()` に `func_index` 引数追加。 |

## テスト計画

### 既存テストの継続パス確認

```bash
cargo test
```

全既存テスト（ユニットテスト + large テスト）が引き続きパスすること。

### 対象テストの手動確認

```bash
cargo test test_scope_scope_static_init_value_persist_001_ws_self -- --nocapture
```

### 追加テスト（任意）

必要に応じて以下の追加テストケースを検討:
- static 配列変数の永続化
- 複数関数がそれぞれ static 変数を持つケース
- static 変数とグローバル変数の共存

## 影響範囲

- `src/compiler_ws/` のみ変更。`semantic_analyzer` や `interpreter` への変更は不要。
- メモリレイアウト上のグローバル領域サイズが拡大するため、`LOCAL_HEAP_BEGIN` / `LOCAL_HEAP_END` の初期値が変わる。
  ヘッダー生成（`generate_header()`）はすでに `ctx.global_heap_size()` を使って動的計算しているため、定数変更は不要。

## 関連

- [symbol-table-design.md](../task/symbol-table-design.md) - ステップ5の実装
- [symbol-table-impl/step5-static-storage-indexing.md](../task/symbol-table-impl/step5-static-storage-indexing.md) - 詳細設計
- `src/interpreter/mod.rs` `initialize_function_statics()` - インタプリタ側の static 変数初期化（参考実装）
