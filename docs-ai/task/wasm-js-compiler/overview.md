# 全体設計

## アーキテクチャ方針

### アプローチ選定

nospace → WASM/JS の実現方法として、2つのアプローチが考えられる:

| アプローチ | 説明 | メリット | デメリット |
|-----------|------|---------|-----------|
| **A: コンパイラバックエンド** | nospace コードを WASM/JS コードに変換 | 出力が小さい、実行が高速 | 実装が複雑 |
| **B: ランタイム WASM 化** | Rust 製インタプリタ自体を WASM にコンパイル | 実装が容易、全機能が自動的に対応 | 出力が大きい (数MB)、実行が低速 |

**本設計ではアプローチ A（コンパイラバックエンド）を採用する。**

理由:
- 既に `compiler_ws` で同じアプローチの実績がある
- 出力サイズが小さく、Web 環境に適している
- 別リポジトリの実行環境が軽量になる
- 将来的な最適化の余地がある

補足: アプローチ B は別途検討可能（wasm-pack で Rust コードを WASM 化し、nospace パーサ + インタプリタを JS から呼び出す方式）。これは「Web Playground」用途として将来検討しても良い。

### モジュール構成

```
src/
├── compiler_js/
│   ├── mod.rs           # 公開 API (compile)
│   ├── context.rs       # コード生成コンテキスト
│   ├── expression.rs    # 式の JS コード生成
│   ├── statement.rs     # 文の JS コード生成
│   └── runtime.rs       # ランタイムコード生成（I/O ブリッジ等）
│
├── compiler_wasm/
│   ├── mod.rs           # 公開 API (compile)
│   ├── context.rs       # コード生成コンテキスト
│   ├── expression.rs    # 式の WASM 命令生成
│   ├── statement.rs     # 文の WASM 命令生成
│   ├── encoder.rs       # WASM バイナリエンコーダ
│   ├── types.rs         # WASM 型定義
│   └── section.rs       # WASM セクション生成
│
├── compile_property.rs  # CompileTarget に Js / Wasm 追加
└── lib.rs               # compile_to_js / compile_to_wasm 追加
```

### 入出力インターフェース

```
compile_to_js:
  入力: &Scope
  出力: Result<String, CompileError>
  備考: 自己完結した JS コード文字列を返す

compile_to_wasm:
  入力: &Scope
  出力: Result<Vec<u8>, CompileError>
  備考: WASM バイナリ (.wasm) を返す
```

### CompileTarget の拡張

```rust
pub enum CompileTarget {
    Ws,        // 既存: Whitespace
    Mnemonic,  // 既存: ニーモニック
    ExWs,      // 既存: 拡張 Whitespace（未対応）
    Json,      // 既存: JSON 中間表現（未対応）
    Js,        // ★新規: JavaScript
    Wasm,      // ★新規: WebAssembly (バイナリ)
    Wat,       // ★新規: WebAssembly Text format（デバッグ用、オプション）
}
```

### CLI 変更

```bash
# JavaScript にコンパイル
nospace20 --mode=compile --target=js source.ns -o output.js

# WASM にコンパイル
nospace20 --mode=compile --target=wasm source.ns -o output.wasm

# WAT にコンパイル（デバッグ用）
nospace20 --mode=compile --target=wat source.ns -o output.wat
```

JS/WASM ターゲットでは `--std=ws` を **要求しない**。
`--std=standard`（デフォルト）で全機能を使用してコンパイル可能。

### バリデーション変更

現在の `CompileProperty::validate()` では `target=ws/mnemonic` の場合 `std=ws` を要求している。
JS/WASM ターゲットではこの制約を適用しない。

```rust
// 変更後のバリデーション(概要)
match self.target {
    CompileTarget::Ws | CompileTarget::Mnemonic => {
        // 既存: std=ws 必須
        if self.std != LanguageStd::Ws { return Err(...); }
    }
    CompileTarget::Js | CompileTarget::Wasm | CompileTarget::Wat => {
        // 新規: std 制約なし
    }
    CompileTarget::ExWs | CompileTarget::Json => {
        // 既存: 未対応
        return Err(...);
    }
}
```

## 共通設計パターン

### コード生成コンテキスト

`compiler_ws` の `CodeGenContext` と同様、スコープ情報を管理するコンテキストを使用する:

```rust
pub struct JsCodeGenContext<'a> {
    scope: &'a Scope,
    indent_level: usize,
    // ...
}

pub struct WasmCodeGenContext<'a> {
    scope: &'a Scope,
    // WASM 固有の状態
    // ...
}
```

### スコープの走査

`compiler_ws` と同じアプローチで `Scope` を走査する:

1. グローバルスコープの変数初期化（`root_statements`）
2. 関数定義の生成
3. `main` 関数のエントリポイント生成

### エラー型

`compiler_ws::CompileError` と同じエラーバリアントを使用（共通化を検討）:

```rust
pub enum CompileError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}
```

将来的に、共通のエラー型として `src/base/` に移動することを検討する。

## 整数のセマンティクス

### nospace の整数

nospace の整数は `i64`（64ビット符号付き整数）である。

### JavaScript での扱い

JavaScript の `Number` 型は IEEE 754 倍精度浮動小数点数であり、整数として安全に表現できる範囲は ±2^53 である。
nospace の i64 全範囲をカバーしない。

**方針**: 基本的には `Number` を使用し、64ビットの範囲全体が必要な場合は `BigInt` へ切り替えるオプションを将来提供する。

理由:
- 多くの nospace プログラムは 53 ビット範囲内で動作する
- `BigInt` はパフォーマンスコストが高い
- 別リポジトリ側で `BigInt` ランタイムを提供することも可能

### WASM での扱い

WASM は `i64` をネイティブサポートしている。nospace の整数セマンティクスと完全に一致する。

ただし JavaScript ↔ WASM のブリッジで `i64` を受け渡す場合、 `BigInt` が必要になる点に注意。
