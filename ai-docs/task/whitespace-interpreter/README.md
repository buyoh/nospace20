# Whitespace インタプリタ (`src/whitespace/interpreter`)

## 概要

Whitespace バイトコードを解釈・実行するインタプリタモジュールを `src/whitespace/` に新規作成する。
明示的スタックマシンとして実装し、任意のタイミングで中断・再開できる設計とする。

## 動機

nospace を Web 上で実行する際、既存の nospace インタプリタ (`src/interpreter/`) を中断可能にするには大規模な書き換えが必要となる（[suspendable-interpreter](../suspendable-interpreter/) 参照）。

一方、以下のアプローチを採ると大幅に簡素化できる:

1. nospace → Whitespace へコンパイル（既存の `compiler_ws` を使用）
2. Whitespace コードを中断可能なインタプリタで実行
3. CLI バイナリ `whitespace20` で Whitespace ファイルを直接実行

Whitespace はフラットな命令セットのスタックマシンであるため、実行状態が全て明示的（データスタック、ヒープ、PC、コールスタック）であり、中断・再開が自然に実現できる。

```
[nospace ソース] → compiler_ws → [Whitespace コード] → whitespace::interpreter → [実行結果]
                                                        ↑ 中断・再開可能
```

## ドキュメント

| ファイル | 内容 |
|---------|------|
| [design.md](design.md) | 全体設計: モジュール構造、公開API、内部状態、中断機構 |
| [module-details.md](module-details.md) | 各モジュールの詳細設計: parser, executor, 拡張API |

## フェーズ計画

### Phase 1: 基本実行エンジン ✅ 完了

- [x] `src/whitespace/` モジュール作成
- [x] Instruction enum の共有方式確定（`compiler_ws` から re-export）
- [x] Whitespace テキスト → 命令列パーサ
- [x] 基本 VM 状態（スタック、ヒープ、PC、コールスタック）
- [x] 全標準命令の実行
- [x] `step(budget)` による中断可能な実行ループ
- [x] Unit テスト（各命令の動作確認）

完了レポート: [whitespace-interpreter-phase1.md](../../done-task/whitespace-interpreter-phase1.md)

### Phase 2: CLI と拡張 API（部分完了）

- [x] `src/bin/whitespace20.rs` CLI バイナリ作成
- [x] 負ヒープアドレスによる拡張 API（`__trace`, `__assert`, `__assert_not`）
- [x] I/O 命令の実装（stdin/stdout バッファ対応）
- [x] `compiler_ws` → `whitespace::interpreter` のパイプライン結合
- [x] `lib.rs` に公開 API 追加

注: 拡張 API の動作確認は compiler_ws の対応が必要

### Phase 3: 統合テスト (wsc 比較)

- [ ] 既存 large テストの Whitespace VM 経由実行
- [ ] wsc と whitespace20 の出力比較テスト
- [ ] `test-manifest.yaml` に `whitespace_vm` ターゲット追加
- [ ] パフォーマンス測定

## 関連タスク

- [suspendable-interpreter/](../suspendable-interpreter/) — nospace インタプリタ自体の中断機能（本タスクにより優先度低下）
- [whitespace-integration-test.md](../whitespace-integration-test.md) — Whitespace コンパイラ統合テスト（wsc を使った既存テスト基盤）

## 関連ドキュメント

- [whitespace-runtime.md](../../architecture/whitespace-runtime.md) — Whitespace 実行環境アーキテクチャ
- [spec-whitespace.md](../../../spec-whitespace.md) — Whitespace 言語仕様
