# Rust 実装設計

Whitespace コンパイラの Rust 実装設計ドキュメント。

## ドキュメント一覧

| ファイル | 内容 |
|----------|------|
| [overview.md](overview.md) | 設計方針・モジュール構成 |
| [whitespace.md](whitespace.md) | Whitespace 命令定義・エンコーダ |
| [memory-label.md](memory-label.md) | メモリレイアウト・ラベル管理 |
| [codegen.md](codegen.md) | コード生成コンテキスト・式・文の生成 |
| [builtin.md](builtin.md) | 組み込みルーチン |
| [api-cli.md](api-cli.md) | 公開 API・CLI 統合 |
| [implementation-plan.md](implementation-plan.md) | テスト戦略・実装計画 |

## クイックリファレンス

### モジュール名

```
src/compiler_ws/    # Whitespace コンパイラモジュール
```

### 処理フロー

```
Scope構造 (semantic_analyzer出力)
    ↓ compiler_ws::compile()
WsProgram
    ↓ to_whitespace()
Whitespace コード文字列
```

### 主要な型

- `HeapAddress` - ヒープアドレス（型安全）
- `LabelId` - ラベル識別子
- `Instruction` - Whitespace 命令
- `WsProgram` - 命令列
- `MemoryLayout` - メモリレイアウト管理
- `LabelAllocator` - ラベル管理
- `CodeGenContext` - コード生成状態
