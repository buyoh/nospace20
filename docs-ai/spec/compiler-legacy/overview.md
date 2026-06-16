# nospace → Whitespace コンパイラ概要

このドキュメントは、旧実装 `.local/nospace/main.cpp` における nospace から Whitespace への変換ロジックを分析・文書化したものです。

## 変換の全体像

nospace コンパイラは以下の流れで動作します：

1. **字句解析 (Tokenize)**: ソースコードをトークン列に分解
2. **構文解析 (Parse)**: トークン列から AST（構文木）を構築
3. **コード生成 (Build)**: AST を Whitespace 命令列に変換

## Whitespace について

Whitespace は空白文字のみで構成されるプログラミング言語です：

| 文字 | 略記 | コード内表現 |
|------|------|--------------|
| スペース (0x20) | SP | `Chr::SP` (0) |
| タブ (0x09) | TB | `Chr::TB` (1) |
| 改行 (0x0A) | LF | `Chr::LF` (2) |

## 出力の構造

生成される Whitespace コードは以下の構造を持ちます：

```
[ヘッダー部]
  - メモリ初期化
  - 組み込みユーティリティルーチン
  
[ユーザーコード部]
  - グローバル変数初期化
  - 関数定義
  
[フッター部]
  - main 関数呼び出し
  - プログラム終了
```

## ドキュメント構成

- [instructions.md](instructions.md) - Whitespace 命令セット
- [memory-layout.md](memory-layout.md) - メモリレイアウトと管理
- [arithmetic.md](arithmetic.md) - 算術・比較演算子
- [logical.md](logical.md) - 論理演算子
- [assignment.md](assignment.md) - 代入演算子
- [control-flow.md](control-flow.md) - 制御構造 (if/while)
- [functions.md](functions.md) - 関数呼び出し
- [io.md](io.md) - I/O 関数
- [builtin-routines.md](builtin-routines.md) - 組み込みルーチン

## 関連ファイル

- 旧実装: `.local/nospace/main.cpp`
- Whitespace 仕様: `docs/spec-whitespace.md`
