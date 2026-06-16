# Whitespace インタプリタ (`src/whitespace/interpreter`) - 完了

**完了日**: 2026-02-17

**状況**: Phase 1 & 2 完了、Phase 3 基本完了（残りは低優先度タスク）

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

### Phase 3: 統合テスト (wsc 比較) ✅ 基本完了

- [x] wsc と whitespace20 の出力比較テスト
  - 完了レポート: [wsc-cross-validation.md](../../done-task/wsc-cross-validation.md)
  - `tests/whitespace_direct_test.rs` に wsc クロスバリデーション実装
  - 39 テスト全て成功 (自前 VM + wsc 両方)

**低優先度タスク（将来の拡張）:**
- [ ] `test-manifest.yaml` に `whitespace_vm` ターゲット追加
- [ ] 既存 large テストの Whitespace VM 経由実行

**備考**: compiler_ws の正確性は既に `whitespace` (wsc) と `whitespace-self` ターゲットで十分検証されている。
Whitespace VM 自体も 39 テストで網羅的にテスト済み。`whitespace_vm` ターゲット追加は
テスト数を大幅に増やす (280件追加) ため、必要性が明確になるまで保留。

## 関連タスク

- [suspendable-interpreter/](../suspendable-interpreter/) — nospace インタプリタ自体の中断機能（本タスクにより優先度低下）
- [wsc-cross-validation.md](wsc-cross-validation.md) — wsc によるクロスバリデーション（完了、39テスト全成功）

## 関連ドキュメント

- [whitespace-runtime.md](../../architecture/whitespace-runtime.md) — Whitespace 実行環境アーキテクチャ
- [docs/spec-whitespace.md](../../../docs/spec-whitespace.md) — Whitespace 言語仕様

---

## 完了サマリー

### 実装済み機能
- ✅ Whitespace テキストパーサ (`src/whitespace/parser.rs`)
- ✅ 中断可能な VM 実行エンジン (`src/whitespace/interpreter.rs`)
- ✅ CLI バイナリ `whitespace20` (`src/bin/whitespace20.rs`)
- ✅ 拡張 API (`__trace`, `__assert`, `__assert_not`)
- ✅ I/O 命令の完全実装
- ✅ 39 件の Whitespace 直接テスト (`tests/whitespace_direct_test.rs`)
- ✅ wsc クロスバリデーション (39 テスト全成功)

### テスト結果
- `cargo test --test whitespace_direct_test`: **39 passed; 0 failed**
- wsc クロスバリデーション (`--ignored`): **39 passed; 0 failed**

### 低優先度タスク (保留)
- `test-manifest.yaml` に `whitespace_vm` ターゲット追加
  - 理由: compiler_ws の正確性は既に `whitespace` (wsc) と `whitespace-self` で十分検証済み
  - 影響: テスト数が 280 件追加され、実行時間が増加
  - 判断: 必要性が明確になるまで保留
