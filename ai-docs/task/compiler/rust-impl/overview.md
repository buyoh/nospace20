# 設計方針・モジュール構成

## 設計方針

### 既存アーキテクチャとの統合

現在の処理パイプライン：

```
ソースコード
    ↓ token_parser
トークン列
    ↓ tree_parser
AST (Statement/Expression)
    ↓ semantic_analyzer
Scope構造
    ↓ interpreter (現在)
実行結果

    ↓ compiler (新規)
Whitespace コード
```

コンパイラは `semantic_analyzer` の出力（`Scope` 構造）を入力として、Whitespace コードを生成します。

### 設計原則

1. **インタプリタと並列に配置** - 既存の interpreter を置き換えず、並列のバックエンドとして実装
2. **型安全性の活用** - Rust の型システムで命令の正当性を保証
3. **適度な抽象化** - デバッグ可能性を維持しつつ型安全なアドレス管理
4. **テスタビリティ** - 各コンポーネントを個別にテスト可能に
5. **エラーハンドリング** - `Result` 型で適切なエラー伝播

## モジュール構成案

```
src/compiler_ws/           # Whitespace コンパイラモジュール
├── mod.rs                 # モジュールエントリポイント
├── types.rs               # 基本型 (WsNumber, LabelId, HeapAddress)
├── instruction.rs         # 命令定義 (Instruction enum)
├── encoder.rs             # バイナリエンコーダ
├── program.rs             # プログラム構造 (WsProgram)
├── memory.rs              # メモリ抽象化レイヤー (MemoryLayout)
├── label.rs               # ラベル管理 (LabelAllocator)
├── builtin.rs             # 組み込みルーチン生成
├── context.rs             # コード生成コンテキスト
├── expression.rs          # 式のコード生成
└── statement.rs           # 文のコード生成
```
│   └── layout.rs       # メモリレイアウト定義
└── label/              # ラベル管理
    └── mod.rs
```

## メインエントリポイント

```rust
// src/compiler/mod.rs

use crate::semantic_analyzer::Scope;

pub use whitespace::WsProgram;

/// コンパイルエラー
#[derive(Debug)]
pub enum CompileError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}

/// Scope を Whitespace プログラムにコンパイル
pub fn compile(scope: &Scope) -> Result<WsProgram, CompileError> {
    let mut ctx = CodeGenContext::new(scope);
    let mut program = WsProgram::new();
    
    // 1. ヘッダー（初期化・組み込みルーチン）を生成
    program.append(builtin::generate_header(&ctx)?);
    
    // 2. グローバルスコープのコードを生成
    program.append(codegen::generate_scope(&mut ctx, scope)?);
    
    // 3. フッター（main呼び出し・終了）を生成
    program.append(builtin::generate_footer(&ctx)?);
    
    Ok(program)
}
```

## 考慮事項

### インタプリタとの相違点

| 項目 | インタプリタ | コンパイラ |
|------|-------------|-----------|
| 変数参照 | 名前で解決 | アドレスに変換 |
| スコープ管理 | 実行時に管理 | コンパイル時に決定 |
| 制御フロー | `Flow` 列挙型 | ラベル/ジャンプ |

### Whitespace の制限

1. **整数のみ**: 浮動小数点数は未サポート
2. **ヒープアクセス**: 直接アドレス指定のみ
3. **スタック操作**: 任意位置へのアクセスは `copy` 命令に限定
