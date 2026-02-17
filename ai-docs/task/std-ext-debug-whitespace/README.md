# `--std-ext debug` によるデバッグ拡張 API の Whitespace 対応

## 概要

`--std-ext debug` オプション指定時に、`__trace`/`__assert`/`__assert_not` を Whitespace コンパイル・実行で有効化する。

現状これらの組み込み関数は Whitespace コンパイル時に noop (引数をそのまま返す) として扱われており、`whitespace20` の VM でも拡張 API は常に有効（`--std-ext` に依存しない）になっているが、仕様上は `--std-ext debug` でゲートされるべきである。

## 仕様

[spec-whitespace.md](../../../spec-whitespace.md) の拡張仕様セクションに記載。

負ヒープアドレスへの Store 命令により拡張 API を呼び出す:

| ヒープアドレス | nospace 関数 | 動作 |
|---|---|---|
| `-1` | `__trace(n)` | `traced[n] += 1` |
| `-2` | `__assert(n)` | `n == 0` → `RuntimeError::AssertionFailed` |
| `-3` | `__assert_not(n)` | `n != 0` → `RuntimeError::AssertionFailed` |

> **注意**: `spec-whitespace.md` の API 仕様テーブルにはアドレス `-10`/`-11`/`-12` と記載されているが、同ドキュメントの詳細仕様セクションおよび既存コード実装ではアドレス `-1`/`-2`/`-3` が使用されている。本設計ではコード実装に合わせて `-1`/`-2`/`-3` を使用する。仕様書のテーブルは修正が必要。

## ドキュメント

| ドキュメント | 内容 |
|---|---|
| [compiler-changes.md](compiler-changes.md) | コンパイラ側の変更設計 (Phase 1) |
| [vm-changes.md](vm-changes.md) | Whitespace VM 側の変更設計 (Phase 2) |
| [api-and-test.md](api-and-test.md) | 公開 API 変更とテスト計画 (Phase 3) |

## Phase 一覧

| Phase | 内容 | 依存 |
|---|---|---|
| Phase 1 | コンパイラ: `--std-ext debug` 時にデバッグ組み込みを負ヒープへの Store として生成 | なし |
| Phase 2 | VM: `--std-ext debug` でのみ負ヒープ拡張 API を有効化 | なし |
| Phase 3 | 公開 API 変更、CLI 接続、テスト追加 | Phase 1, 2 |

## 現状分析

### コンパイラ側 (`compiler_ws`)
- `expression.rs`: `generate_builtin_debug_noop` で `__trace`/`__assert`/`__assert_not`/`__clog` を全て noop 処理
- `compile()` 関数は `&Scope` のみ受け取り、`CompileProperty` や `target_extensions` は受け取らない
- `CodeGenContext` にも拡張フラグなし

### VM 側 (`whitespace/interpreter.rs`)
- `heap_store` が `-1`/`-2`/`-3` を常に拡張 API として処理（`--std-ext` に依存しない）
- `WhitespaceVM` にオプションフラグなし

### CLI 側
- `nospace20`: `CompileProperty.target_extensions` に `Debug` を格納するが、コンパイラに渡さない
- `whitespace20`: `--std-ext` を受け付けるが未使用（コメントに「将来の拡張のため」と記載）

### 公開 API (`lib.rs`)
- `compile_to_whitespace(&Scope)`: 拡張情報を受け取らない
- `compile_to_whitespace_debug(&Scope)`: 同上
