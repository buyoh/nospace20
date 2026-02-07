# WASM コンパイラ詳細設計

## 概要

nospace の `Scope`（意味解析結果）を入力として、WebAssembly バイナリ（`.wasm`）を出力する。
出力された WASM モジュールは、ホスト環境（JavaScript）から I/O 関数をインポートすることで実行可能。

WASM Text format（WAT）の出力もデバッグ用にサポートする。

## WASM モジュールの全体構造

```wat
(module
  ;; === Import Section ===
  ;; ホスト環境から I/O 関数をインポート
  (import "env" "putInt" (func $__putInt (param i64) (result i64)))
  (import "env" "putChar" (func $__putChar (param i64) (result i64)))
  (import "env" "getInt" (func $__getInt (result i64)))
  (import "env" "getChar" (func $__getChar (result i64)))

  ;; === Memory Section ===
  ;; グローバル変数・static 変数用のリニアメモリ
  (memory (export "memory") 1)

  ;; === Global Section ===
  ;; グローバル変数を WASM globals で管理
  (global $__g_0 (mut i64) (i64.const 0))
  (global $__g_1 (mut i64) (i64.const 0))

  ;; === Function Section ===
  (func $add (param $a i64) (param $b i64) (result i64)
    local.get $a
    local.get $b
    i64.add
  )

  (func $main (result i64)
    (local $__v_0 i64)
    ;; ...
    i64.const 0
  )

  ;; === Start Section ===
  ;; グローバル初期化 + main 呼び出し
  (func $__start
    ;; root_statements の実行
    ;; main の呼び出し
    call $main
    drop
  )
  (start $__start)

  ;; === Export Section ===
  (export "main" (func $main))
  (export "__start" (func $__start))
)
```

## 実装アプローチ: `wasm-encoder` crate の使用

WASM バイナリを手動でエンコードするのは複雑なため、`wasm-encoder` crate の使用を推奨する。

### 利点

- WASM バイナリフォーマットの正確なエンコード
- セクション構築の API が直感的
- bytecodealliance が公式メンテナンス（wasmtime と同じ組織）
- 軽量な依存（ランタイム不要、エンコードのみ）

### 代替案: 手動エンコード

`wasm-encoder` を使わない場合、`src/compiler_wasm/encoder.rs` で LEB128 エンコーディング等を独自実装する。
`compiler_ws` の `encoder.rs` に類似したアプローチとなる。

**決定**: 初期は `wasm-encoder` crate を使用。依存を最小限にしたい場合は後から独自実装に切り替え可能。

### Cargo.toml 変更

```toml
[dependencies]
wasm-encoder = "0.225"  # バージョンは実装時の最新を使用
```

`wasm-encoder` は オプショナル依存 (feature flag) にすることも検討:

```toml
[features]
wasm = ["dep:wasm-encoder"]

[dependencies]
wasm-encoder = { version = "0.225", optional = true }
```

## 型マッピング

| nospace | WASM |
|---------|------|
| 整数 (i64) | `i64` |
| 真偽値 (0/非0) | `i64`（0 = false、非0 = true） |
| 関数の戻り値 | `i64`（常に i64 を返す） |
| void（return なし） | `i64`（0 を返す） |

nospace は型システムが未実装で全て i64 のため、WASM の型マッピングはシンプル。

## 変数のメモリモデル

### ローカル変数

WASM の `local` を使用する。各関数内で必要な分だけ `local` を宣言。

```wat
(func $example (result i64)
  (local $__v_0 i64)  ;; ローカル変数 0
  (local $__v_1 i64)  ;; ローカル変数 1
  ;; ...
)
```

### 関数引数

WASM の `param` を使用。nospace の引数と直接マッピングする。

```wat
(func $add (param $a i64) (param $b i64) (result i64)
  local.get $a
  local.get $b
  i64.add
)
```

### グローバル変数・static 変数

WASM の `global` を使用する。

```wat
(global $__g_0 (mut i64) (i64.const 0))
(global $__g_1 (mut i64) (i64.const 0))
```

`IdentifierRef.is_global == true` の場合は `global.get` / `global.set` を使用。
`IdentifierRef.is_global == false` の場合は `local.get` / `local.set` を使用。

### ブロックスコープの変数

WASM にはブロックスコープの概念がないため、ブロックスコープ内の変数もフラットに `local` として宣言する。

nospace:
```
func: example() {
  let: a;
  if: 1 {
    let: b;
    b = 42;
  };
}
```

WASM:
```wat
(func $example (result i64)
  (local $__v_0 i64)  ;; a（関数スコープ）
  (local $__v_1 i64)  ;; b（ブロックスコープだが local としてフラット化）
  ;; ...
)
```

ローカル変数のインデックスは、関数ブロックとその子孫ブロック全体で通しナンバーを割り当てる。
`IdentifierRef` の `scope_depth` と `local_index` から、関数内でのフラットなインデックスを計算する必要がある。

#### フラット化のインデックス計算

コード生成コンテキストで、スコープに入るたびにオフセットを記録する:

```rust
struct WasmCodeGenContext {
    /// 現在のスコープのローカル変数オフセット
    /// scope_depth → base_offset のマップ
    scope_offsets: Vec<usize>,
    /// 次に割り当てるローカルインデックス
    next_local_index: usize,
}

// IdentifierRef → WASM local index
fn resolve_local(&self, id_ref: &IdentifierRef) -> usize {
    let depth = self.scope_offsets.len() - 1 - id_ref.scope_depth;
    self.scope_offsets[depth] + id_ref.local_index
}
```

## 式の変換規則

WASM はスタックマシンのため、式はスタックに値をプッシュする形で生成する。

### 算術演算

| nospace | WASM 命令列 |
|---------|------------|
| `a + b` | `[a] [b] i64.add` |
| `a - b` | `[a] [b] i64.sub` |
| `a * b` | `[a] [b] i64.mul` |
| `a / b` | `[a] [b] i64.div_s` (符号付き除算) |
| `a % b` | `[a] [b] i64.rem_s` (符号付き剰余) |
| `-a` | `i64.const 0  [a]  i64.sub` |

### 比較演算

nospace の比較演算は結果が 0 or 1 の `i64` である。
WASM の比較演算は `i32` を返すため、`i64.extend_i32_s` で拡張する。

| nospace | WASM 命令列 |
|---------|------------|
| `a == b` | `[a] [b] i64.eq  i64.extend_i32_s` |
| `a != b` | `[a] [b] i64.ne  i64.extend_i32_s` |
| `a < b` | `[a] [b] i64.lt_s  i64.extend_i32_s` |
| `a <= b` | `[a] [b] i64.le_s  i64.extend_i32_s` |
| `a > b` | `[a] [b] i64.gt_s  i64.extend_i32_s` |
| `a >= b` | `[a] [b] i64.ge_s  i64.extend_i32_s` |

### 論理演算

| nospace | WASM 命令列 | 備考 |
|---------|-----------|------|
| `!a` | `[a] i64.eqz  i64.extend_i32_s` | 0→1, 非0→0 |
| `a && b` | 下記参照 | 短絡評価 |
| `a \|\| b` | 下記参照 | 短絡評価 |

#### 短絡評価の実現

WASM の `if` 構造制御命令を使って短絡評価を実現する:

```wat
;; a && b
[a]                           ;; a をスタックに
i64.eqz                       ;; a == 0 ?
if (result i64)               ;; a が 0 なら
  i64.const 0                 ;; 0 を返す（b を評価しない）
else
  [b]                         ;; b を評価
  i64.const 0
  i64.ne                      ;; b != 0 ?
  i64.extend_i32_s            ;; 0 or 1
end

;; a || b
[a]                           ;; a をスタックに
local.tee $__tmp              ;; a を一時変数に保存しつつスタックにも残す
i64.const 0
i64.ne                        ;; a != 0 ?
if (result i64)               ;; a が非0 なら
  local.get $__tmp            ;; a の値を返す（b を評価しない）
else
  [b]                         ;; b を評価して返す
end
```

### 代入式

代入は式として値を返す（代入される値を返す）:

```wat
;; x = expr
[expr]
local.tee $x    ;; 値を代入しつつスタックにも残す（式として使う場合）
;; 文として使う場合は tee ではなく set を使い、戻り値不要なら drop
```

## 制御構文の変換

### if 文

```wat
;; if: cond { then_block } else: { else_block };
[cond]
i64.const 0
i64.ne                  ;; cond != 0 → i32 の 0/1
if
  ;; then_block
else
  ;; else_block
end
```

if/else:if チェーン:

```wat
[cond1]
i64.const 0
i64.ne
if
  ;; then_block1
else
  [cond2]
  i64.const 0
  i64.ne
  if
    ;; then_block2
  else
    ;; else_block
  end
end
```

### while 文

WASM には `while` がないため、`block` + `loop` + `br_if` で実現する:

```wat
;; while: cond { body };
block $break_label
  loop $continue_label
    [cond]
    i64.eqz
    br_if $break_label      ;; cond == 0 なら抜ける
    ;; body
    br $continue_label       ;; ループ先頭に戻る
  end
end
```

### break / continue

- `break` → `br $break_label`（最も近い `block` に分岐）
- `continue` → `br $continue_label`（最も近い `loop` に分岐）

ネストしたループの場合、ラベルの深さを追跡する。

```rust
struct WasmCodeGenContext {
    /// break 先のラベル深さスタック
    break_labels: Vec<u32>,
    /// continue 先のラベル深さスタック
    continue_labels: Vec<u32>,
    /// 現在のブロック深さ
    block_depth: u32,
}
```

### return

```wat
;; return: expr;
[expr]
return
```

## 関数の生成

### 関数定義

各 nospace 関数を WASM 関数として生成:

```wat
(func $func_name (param $arg0 i64) (param $arg1 i64) (result i64)
  (local $__v_0 i64)     ;; ローカル変数
  (local $__v_1 i64)
  ;; body statements
  i64.const 0             ;; return なしの場合のデフォルト戻り値
)
```

### ネストした関数

WASM は関数のネストをサポートしない。
nospace のネストした関数は、モジュールレベルにフラット化して配置する。

名前衝突を避けるため、スコープパスでプレフィックスを付ける:

```
func: outer() {
  func: inner() { ... }
}
```

→

```wat
(func $outer (result i64) ...)
(func $outer__inner (result i64) ...)
```

### ホイスティング

WASM モジュールの関数は定義順に依存しないため、ホイスティングは自然に実現される。

### エントリポイント

`$__start` 関数を生成し、WASM の `start` セクションに登録する:

```wat
(func $__start
  ;; グローバル変数の初期化（root_statements）
  ;; ...
  
  ;; main の呼び出し
  call $main
  drop    ;; main の戻り値は破棄
)
(start $__start)
```

別リポジトリ側で main の戻り値を取得したい場合は、`$main` を export して JS から直接呼ぶ。

## 組み込み関数

### I/O 関数（インポート）

```wat
(import "env" "putInt"  (func $__putInt  (param i64) (result i64)))
(import "env" "putChar" (func $__putChar (param i64) (result i64)))
(import "env" "getInt"  (func $__getInt  (result i64)))
(import "env" "getChar" (func $__getChar (result i64)))
```

インポート関数の戻り値仕様は [io-interface.md](io-interface.md) を参照。

### デバッグ関数

| nospace | WASM 生成コード | 備考 |
|---------|---------------|------|
| `__clog(x)` | `[x] (インポート呼び出し or noop)` | オプションで import |
| `__trace(key)` | `i64.const 0` (noop) | WASM では無視 |
| `__assert(x)` | `[x]` + trap 判定 | `unreachable` 命令で trap |
| `__assert_not(x)` | `[x]` + trap 判定 | |

assert の WASM 実装:

```wat
;; __assert(x): x == 0 なら trap
[x]
local.tee $__tmp
i64.eqz
if
  unreachable   ;; trap (WASM 実行停止)
end
local.get $__tmp  ;; x を返す
```

## モジュール設計

### mod.rs

```rust
pub fn compile(scope: &Scope) -> Result<Vec<u8>, CompileError> {
    let mut ctx = WasmCodeGenContext::new(scope);
    
    // 1. 関数シグネチャの収集
    ctx.collect_functions(scope)?;
    
    // 2. インポートセクション生成
    // 3. グローバルセクション生成
    // 4. 関数セクション生成
    // 5. エントリポイント (__start) 生成
    // 6. エクスポートセクション生成
    // 7. エンコード
    
    ctx.encode()
}

/// WAT (テキスト形式) で出力（デバッグ用）
pub fn compile_to_wat(scope: &Scope) -> Result<String, CompileError> {
    // WAT テキストを直接生成するか、
    // wasmprinter crate でバイナリから変換
    todo!()
}
```

### context.rs

```rust
pub struct WasmCodeGenContext<'a> {
    scope: &'a Scope,
    
    /// 関数テーブル: nospace 関数名 → WASM 関数インデックス
    function_table: BTreeMap<String, u32>,
    
    /// グローバル変数テーブル: IdentifierRef → WASM global インデックス
    global_count: u32,
    
    /// 現在のローカル変数情報
    local_count: u32,
    scope_offsets: Vec<usize>,
    
    /// break/continue ラベル管理
    break_labels: Vec<u32>,
    continue_labels: Vec<u32>,
    block_depth: u32,
}
```

### expression.rs

`ExecExpression` を WASM 命令列に変換する関数群。
命令列は `Vec<Instruction>` のような中間表現として蓄積し、最終的に `wasm-encoder` に渡す。

### statement.rs

`ExecStatement` を WASM 命令列に変換する関数群。

### encoder.rs

`wasm-encoder` crate を使用して WASM バイナリをエンコードする。
`wasm-encoder` を使わない場合は、独自の LEB128 エンコーダと WASM セクション構築を実装。

## テスト方針

### 単体テスト

式・文の変換が正しい WASM 命令列を生成することを検証。

### 統合テスト

1. nospace ソース → WASM コンパイル → WASM 実行 → 出力比較
2. WASM 実行には `wasmtime`（CLI）または Node.js の `WebAssembly` API を使用

統合テストのパターン:

```bash
# パターン A: wasmtime で実行
nospace20 --mode=compile --target=wasm source.ns -o output.wasm
wasmtime output.wasm

# パターン B: Node.js で実行
nospace20 --mode=compile --target=wasm source.ns -o output.wasm
node run_wasm.js output.wasm
```

パターン B（Node.js）の方が、I/O ブリッジのテストが容易であるため推奨。

### 既存テストケースの流用

`resources/tests/` のテストケースのうち、I/O を使用するもの（`success_io` カテゴリ）を
WASM 統合テストにも流用する。

## 制限事項・既知の課題

1. **再帰の深さ**: WASM のコールスタックは有限（ブラウザ依存、通常数千フレーム）
2. **整数オーバーフロー**: WASM の `i64` は C と同じラップアラウンド動作。nospace のインタプリタと一致する。
3. **浮動小数点**: nospace に浮動小数点はないため、WASM の `f32`/`f64` は使用しない。
4. **メモリ**: リニアメモリは将来の配列実装で使用予定。現時点では globals のみ。
