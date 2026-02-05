# CLI コンパイルオプション設計

Whitespace コンパイラ実装を踏まえた CLI オプションの設計。

## 概要

コンパイルプロパティ（`CompileProperty`）を導入し、CLI 引数から動作モード・ターゲット・言語サブセットを指定可能にする。

## CLI 引数

### 現在の引数

```
nospace20 [OPTIONS] [FILE]

Arguments:
  [FILE]  Source file to execute (reads from stdin if not provided)

Options:
  -d, --debug    Show trace results after execution
  -h, --help     Print help
  -V, --version  Print version
```

### 追加する引数

```
nospace20 [OPTIONS] [FILE]

Arguments:
  [FILE]  Source file to execute (reads from stdin if not provided)

Options:
      --std <STD>        Language subset [default: standard]
                         [possible values: standard, min, ws]
      --mode <MODE>      Execution mode [default: run]
                         [possible values: run, compile]
      --target <TARGET>  Compile target (only with --mode=compile)
                         [possible values: ws, mnemonic, ex-ws, json]
  -o, --output <FILE>    Output file (only with --mode=compile, stdout if not specified)
  -d, --debug            Show trace results after execution
  -h, --help             Print help
  -V, --version          Print version
```

## オプション詳細

### `--std` : 言語サブセット

言語機能のサブセットを指定する。

| 値 | 説明 | 状態 |
|---|---|---|
| `standard` | 全ての機能が有効（デフォルト） | ✅ 実装済み |
| `min` | 最小限の機能セット | ❌ 未対応 |
| `ws` | Whitespace コンパイル互換 | ✅ 実装済み |

**用途:**

- `standard` : 通常の開発・実行
- `min` : セルフホスティングコンパイラ構築時に使用。言語仕様の最小サブセットのみを使用し、そのサブセットでコンパイラ自身を実装可能にする。
- `ws` : Whitespace へのコンパイル時に選択。ビット演算等の Whitespace では実装困難な機能を制限する。

### `--mode` : 実行モード

| 値 | 説明 | 状態 |
|---|---|---|
| `run` | インタプリタモード（デフォルト） | ✅ 実装済み |
| `compile` | コンパイルモード | ✅ 実装済み（要CLI統合） |

### `--target` : コンパイルターゲット

`--mode=compile` 時のみ有効。

| 値 | 説明 | `--std` 制約 | 状態 |
|---|---|---|---|
| `ws` | Whitespace へコンパイル | `ws` のみ | ✅ 実装済み |
| `mnemonic` | ニーモニック表記へコンパイル | `ws` のみ | ✅ 実装済み |
| `ex-ws` | 拡張 Whitespace へコンパイル | なし | ❌ 未対応 |
| `json` | 中間表現 (JSON) へコンパイル | なし | ❌ 未対応 |

## CompileProperty 構造体

```rust
/// 言語サブセット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageStd {
    #[default]
    Standard,
    Min,
    Ws,
}

/// 実行モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Run,
    Compile,
}

/// コンパイルターゲット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompileTarget {
    #[default]
    Ws,
    Mnemonic,
    ExWs,
    Json,
}

/// コンパイルプロパティ
/// 
/// CLI 引数から構築され、各処理段階に渡される設定情報。
#[derive(Debug, Clone, Default)]
pub struct CompileProperty {
    /// 言語サブセット
    pub std: LanguageStd,
    /// 実行モード
    pub mode: ExecutionMode,
    /// コンパイルターゲット（mode=Compile 時のみ使用）
    pub target: CompileTarget,
    /// 出力ファイルパス（mode=Compile 時、None なら stdout）
    pub output: Option<String>,
    /// デバッグモード
    pub debug: bool,
}
```

## CLI 引数の Rust 定義

```rust
use clap::{Parser, ValueEnum};

/// 言語サブセット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum CliStd {
    #[default]
    Standard,
    Min,
    Ws,
}

/// 実行モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum CliMode {
    #[default]
    Run,
    Compile,
}

/// コンパイルターゲット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum CliTarget {
    #[default]
    Ws,
    Mnemonic,
    ExWs,
    Json,
}

/// nospace - A nospace language interpreter and compiler
#[derive(Parser, Debug)]
#[command(name = "nospace20")]
#[command(version = "0.1.0")]
#[command(about = "A nospace language interpreter and compiler", long_about = None)]
struct Args {
    /// Source file to execute (reads from stdin if not provided)
    file: Option<String>,

    /// Language subset
    #[arg(long, value_enum, default_value_t = CliStd::Standard)]
    std: CliStd,

    /// Execution mode
    #[arg(long, value_enum, default_value_t = CliMode::Run)]
    mode: CliMode,

    /// Compile target (only with --mode=compile)
    #[arg(long, value_enum, default_value_t = CliTarget::Ws)]
    target: CliTarget,

    /// Output file (only with --mode=compile, stdout if not specified)
    #[arg(short, long)]
    output: Option<String>,

    /// Show trace results after execution
    #[arg(short, long)]
    debug: bool,
}
```

## バリデーションルール

1. `--mode=run` の場合、`--target` と `--output` は無視される
2. `--mode=compile --target=ws` または `--target=mnemonic` の場合、`--std=ws` が必須
3. `--std=min` は現在未対応（エラー）
4. `--target=ex-ws` または `--target=json` は現在未対応（エラー）

## 処理フロー

```
┌─────────────────────────────────────────────────────────────┐
│                       CLI 引数解析                          │
│  Args → CompileProperty                                     │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      バリデーション                          │
│  - std/mode/target の組み合わせチェック                      │
│  - 未対応機能のエラー                                        │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      ソースコード読込                        │
│  file or stdin → code_raw                                   │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      パース処理                              │
│  parse_to_tokens → parse_to_tree → syntactic_analyze        │
│  ※ property.std に応じた機能制限チェックを追加              │
└────────────────────────────┬────────────────────────────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
              ▼                             ▼
┌─────────────────────────┐   ┌─────────────────────────────┐
│     mode = Run          │   │       mode = Compile        │
│  interpret_with_env     │   │                             │
│  結果表示               │   │  target に応じてコンパイル  │
└─────────────────────────┘   │  - ws: to_whitespace()      │
                              │  - mnemonic: to_debug_str() │
                              │  出力（file or stdout）     │
                              └─────────────────────────────┘
```

## 実装計画

### Phase 1: 基本的な CLI 統合

1. `CompileProperty` 構造体を `src/lib.rs` に追加
2. CLI 引数定義を更新
3. バリデーション実装
4. `--mode=compile --target=ws/mnemonic` の動作確認

### Phase 2: 言語サブセット制限

1. `--std=ws` 時の機能制限チェック実装
   - ビット演算の禁止
   - その他 Whitespace で実現困難な機能の制限

### Phase 3: 追加ターゲット（将来）

1. `--target=json` 中間表現出力
2. `--target=ex-ws` 拡張 Whitespace
3. `--std=min` 最小サブセット

## 使用例

```bash
# インタプリタモード（デフォルト）
nospace20 program.ns

# Whitespace へコンパイル
nospace20 --std=ws --mode=compile --target=ws program.ns > program.ws

# ニーモニック表記へコンパイル（デバッグ用）
nospace20 --std=ws --mode=compile --target=mnemonic program.ns

# 出力ファイル指定
nospace20 --std=ws --mode=compile --target=ws -o program.ws program.ns

# デバッグモードで実行
nospace20 --debug program.ns
```

## エラーメッセージ例

```
error: --target=ws requires --std=ws
  tip: use `--std=ws --mode=compile --target=ws`

error: --std=min is not yet implemented

error: --target=ex-ws is not yet implemented
```
