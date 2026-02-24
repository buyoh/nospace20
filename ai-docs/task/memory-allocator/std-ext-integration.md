# `--std-ext alloc` の統合と条件分岐

## 概要

`--std-ext alloc` オプションを既存の `--std-ext debug` と同様のパターンで統合する。

## CLI 引数の追加

### nospace20.rs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliTargetExt {
    /// デバッグ拡張
    Debug,
    /// メモリアロケータ拡張
    Alloc,
}
```

使用例:
```bash
cargo run --bin nospace20 -- --std=ws --mode=compile --target=ws --std-ext alloc input.ns
cargo run --bin nospace20 -- --std=ws --mode=compile --target=ws --std-ext debug --std-ext alloc input.ns
```

### README.md の更新

```markdown
- `--std-ext`
  - `debug` : デバッグ、assertion関数を有効にする
  - `alloc` : メモリアロケータを有効にする（`__alloc`/`__free` 組み込み関数、動的フレーム管理）
```

## CompileProperty への統合

### compile_property.rs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetExtension {
    Debug,
    Alloc,
}
```

バリデーションの追加:

```rust
impl CompileProperty {
    pub fn validate(&self) -> Result<(), String> {
        // ... 既存のバリデーション ...

        // --std-ext alloc は --mode=compile --std=ws 時のみ
        if self.target_extensions.contains(&TargetExtension::Alloc) {
            if self.mode != ExecutionMode::Compile || self.std != LanguageStd::Ws {
                return Err("--std-ext alloc requires --mode=compile --std=ws".to_string());
            }
        }

        Ok(())
    }
}
```

## コンパイラへの伝播

### 現在の `--std-ext debug` の伝播経路 (参考)

```
CLI: args.std_ext → CompileProperty.target_extensions
     ↓
lib.rs: target_extensions.contains(&Debug) → debug_ext: bool
     ↓
compiler_ws/mod.rs: compile_with_options(scope, debug_ext)
     ↓
context.rs: CodeGenContext { debug_ext, ... }
     ↓
expression.rs: ctx.is_debug_ext() → 条件分岐
```

### `--std-ext alloc` の伝播経路 (同パターン)

```
CLI: args.std_ext → CompileProperty.target_extensions
     ↓
lib.rs: target_extensions.contains(&Alloc) → alloc_ext: bool
     ↓
compiler_ws/mod.rs: compile_with_options(scope, debug_ext, alloc_ext)
     ↓
context.rs: CodeGenContext { alloc_ext, ... }
     ↓
builtin.rs: ctx.is_alloc_ext() → ヘッダー生成分岐
statement.rs: ctx.is_alloc_ext() → フレーム管理分岐
expression.rs: ctx.is_alloc_ext() → __alloc/__free 生成
```

## 条件分岐ポイント一覧

| 場所 | 条件 | 動作 |
|---|---|---|
| `builtin.rs: generate_header` | `alloc_ext` | アロケータメタデータの初期化追加 |
| `builtin.rs: generate_allocator_runtime` | `alloc_ext` | サブルーチン生成（alloc 時のみ） |
| `builtin.rs: generate_local_allocate` | `alloc_ext` | バンプ方式 / alloc 呼び出し方式を分岐 |
| `builtin.rs: generate_local_deallocate` | `alloc_ext` | 復元方式 / free 呼び出し方式を分岐 |
| `statement.rs: generate_function_definition` | `alloc_ext` | 引数配置のタイミング変更 |
| `expression.rs: generate_function_call` | `alloc_ext` | `__alloc`/`__free` の認識・コード生成 |

## `--std-ext debug` との複合

`--std-ext debug --std-ext alloc` は両方同時に有効化可能。各機能は独立して動作:

- `debug_ext`: デバッグ組み込み関数 (`__trace` 等) の負ヒープアドレス Store 生成
- `alloc_ext`: アロケータサブルーチン生成、フレーム管理方式変更、`__alloc`/`__free` 生成

干渉する部分はない（メタデータアドレスも重複しない: debug は負アドレス、alloc は正アドレス 5,6）。

## whitespace20 (VM) への影響

### 現時点での対応

whitespace20 (Whitespace VM) 側では `--std-ext alloc` の特別な対応は**不要**。

理由:
- アロケータは標準 Whitespace 命令セット（Push, Store, Retrieve, Call, Return, Jump 等）のみで実装される
- VM 側にフックや特殊処理は必要ない
- メモリ管理は全てコンパイル時に生成されるサブルーチンで完結

### 将来のデバッグ支援 (オプション)

将来的に `--std-ext alloc` と `--std-ext debug` の併用時に、VM 側でメモリ関連のエラー検出を行うことを検討:

| 機能 | 方式 | 優先度 |
|---|---|---|
| 解放済みメモリアクセス検出 | アロケータがメモリ poison 値を書き込み | 低 |
| ダブルフリー検出 | フリーリストの整合性チェック | 低 |
| メモリリーク報告 | 終了時にヒープウォーク | 低 |

これらは初期実装のスコープ外。
