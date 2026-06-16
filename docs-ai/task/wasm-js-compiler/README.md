# nospace → WASM / JavaScript コンパイラ

## 概要

nospace ソースコードを WebAssembly (WASM) および JavaScript にコンパイルする機能を追加する。
現在の CLI が持つインタプリタ実行と同等の動作を、WASM/JS の実行で実現する。

JavaScript の実行環境および UI は別リポジトリで開発予定。本リポジトリでは **コンパイラバックエンド** のみを担当する。

## ドキュメント

| ファイル | 内容 |
|---------|------|
| [overview.md](overview.md) | 全体設計・アーキテクチャ |
| [javascript-compiler.md](javascript-compiler.md) | JavaScript コンパイラ詳細設計 |
| [wasm-compiler.md](wasm-compiler.md) | WASM コンパイラ詳細設計 |
| [io-interface.md](io-interface.md) | I/O インターフェース・ランタイム仕様 |

## フェーズ計画

### Phase 1: 基盤整備

- [ ] `CompileTarget` に `Js` / `Wasm` を追加
- [ ] CLI バリデーション更新（`--std=ws` 不要にする等）
- [ ] `compiler_js/` モジュールスケルトン作成
- [ ] `compiler_wasm/` モジュールスケルトン作成
- [ ] I/O インターフェース仕様の確定
- [ ] `lib.rs` に `compile_to_js` / `compile_to_wasm` 公開 API 追加

### Phase 2: JavaScript コンパイラ

- [ ] 式のコード生成（四則演算、比較、論理演算）
- [ ] 文のコード生成（変数代入、return、break、continue）
- [ ] 制御構文（if/while）のコード生成
- [ ] 関数定義・呼び出しのコード生成
- [ ] 組み込み関数の I/O ブリッジ生成
- [ ] グローバル変数・static 変数のサポート
- [ ] テストケース作成・統合テスト

### Phase 3: WASM コンパイラ

- [ ] WASM バイナリエンコーダ実装（または `wasm-encoder` crate 導入）
- [ ] 型セクション・関数セクション生成
- [ ] 式のコード生成（i64 演算）
- [ ] 制御構文の WASM 構造化制御フローへの変換
- [ ] 関数定義・呼び出しのコード生成
- [ ] I/O インポート関数の定義
- [ ] メモリレイアウト設計（グローバル変数）
- [ ] テストケース作成・統合テスト

### Phase 4: 統合・仕上げ

- [ ] 既存テストケースでの WASM/JS 出力の検証
- [ ] エラーハンドリングの統一
- [ ] ドキュメント更新

## 対象とする言語機能

現在 interpreter が対応している機能のうち、以下を対象とする:

| 機能 | JS | WASM | 備考 |
|------|:--:|:----:|------|
| 整数リテラル (10進/16進) | ○ | ○ | |
| 文字リテラル | ○ | ○ | |
| 四則演算・剰余 | ○ | ○ | |
| 比較演算子 | ○ | ○ | |
| 論理演算子 (短絡評価) | ○ | ○ | |
| 変数宣言・代入 | ○ | ○ | |
| if / else / else:if | ○ | ○ | |
| while | ○ | ○ | |
| break / continue | ○ | ○ | |
| 関数定義・呼び出し | ○ | ○ | |
| return | ○ | ○ | |
| グローバル変数 | ○ | ○ | |
| static 変数 | ○ | ○ | |
| ブロックスコープ | ○ | ○ | |
| `__puti` / `__putc` | ○ | ○ | I/O ブリッジ経由 |
| `__geti` / `__getc` | ○ | ○ | I/O ブリッジ経由 |
| `__clog` / `__trace` | ○ | ○ | デバッグ用、noop or ブリッジ |
| `__assert` / `__assert_not` | ○ | ○ | デバッグ用 |

## 対象としない機能（未実装の言語機能）

以下は nospace 言語仕様に記載されているが、interpreter でも未実装のため対象外:

- 配列
- 文字列
- 参照・間接参照演算子
- 複合代入演算子
- 型システム
- final / const 変数
- 変数の初期値指定

## 現状のパイプラインと変更点

```
現状:
  source → token_parser → tree_parser → semantic_analyzer → Scope
                                                              ├→ interpreter (実行)
                                                              └→ compiler_ws (→ Whitespace)

変更後:
  source → token_parser → tree_parser → semantic_analyzer → Scope
                                                              ├→ interpreter (実行)
                                                              ├→ compiler_ws  (→ Whitespace)
                                                              ├→ compiler_js  (→ JavaScript)  ★新規
                                                              └→ compiler_wasm(→ WASM)        ★新規
```

共通のフロントエンド（字句解析→構文解析→意味解析）はそのまま共有し、
`Scope` を入力として新しいバックエンドを追加する。
これは `compiler_ws` と同じパターンである。
