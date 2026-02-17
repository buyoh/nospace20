# `--std-ext debug` によるデバッグ拡張 API の Whitespace 対応

## 概要

`--std-ext debug` オプション指定時に、`__trace`/`__assert`/`__assert_not` を Whitespace コンパイル・実行で有効化する。

現状これらの組み込み関数は Whitespace コンパイル時に noop (引数をそのまま返す) として扱われており、`whitespace20` の VM でも拡張 API は常に有効（`--std-ext` に依存しない）になっているが、仕様上は `--std-ext debug` でゲートされるべきである。

## 仕様

[spec-whitespace.md](../../../spec-whitespace.md) の拡張仕様セクションに記載。

負ヒープアドレスへの Store 命令により拡張 API を呼び出す:

| ヒープアドレス | nospace 関数 | 動作 |
|---|---|---|
| `-10` | `__trace(n)` | `traced[n] += 1` |
| `-11` | `__assert(n)` | `n == 0` → `RuntimeError::AssertionFailed` |
| `-12` | `__assert_not(n)` | `n != 0` → `RuntimeError::AssertionFailed` |

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

## 実装進捗

### 2026-02-17: 実装完了

**Phase 1: コンパイラ変更**
- ✅ `memory.rs`: 拡張 API アドレス定数 (`EXT_TRACE_ADDR`, `EXT_ASSERT_ADDR`, `EXT_ASSERT_NOT_ADDR`) を追加
- ✅ `context.rs`: `CodeGenContext` に `debug_ext` フラグと `is_debug_ext()` アクセサを追加
- ✅ `mod.rs`: `compile_with_options(scope, debug_ext)` 関数を追加、既存の `compile()` は従来互換として維持
- ✅ `expression.rs`: デバッグ組み込み関数のコード生成を条件分岐に変更、`generate_builtin_debug_store()` 関数を追加

**Phase 2: VM 変更**
- ✅ `interpreter.rs`: `WhitespaceVM` に `debug_ext` フラグと `with_debug_ext()` ビルダーメソッドを追加
- ✅ `heap_store()`: `debug_ext` が true の場合のみ負ヒープアドレスを拡張 API として処理
- ✅ `whitespace20.rs`: `--std-ext debug` を VM に渡すように変更
- ✅ Unit テスト `test_trace_extension` を修正して `.with_debug_ext(true)` を呼び出すように変更

**Phase 3: 公開 API・CLI・WASM 変更**
- ✅ `lib.rs`: `compile_to_whitespace_with_options()` と `compile_to_whitespace_debug_with_options()` を追加
- ✅ `nospace20.rs`: `target_extensions` をコンパイラに渡すように変更
- ✅ `wasm_api.rs`: import に新しい API を追加、`from_whitespace()` で `.with_debug_ext(false)` を明示的に呼び出すように変更

**Phase 4: テストケース追加**
- ✅ `debug_assert_pass_001`: `__assert` が非ゼロ値で正常に通過することを確認
- ✅ `debug_assert_not_pass_001`: `__assert_not` がゼロ値で正常に通過することを確認
- ✅ `debug_trace_multi_001`: 複数の `__trace` が正しくカウントされることを確認
- ✅ `test-manifest.yaml` に 3 つのテストを追加

**Phase 5: テスト実行**
- ✅ 新規テストが全て成功
- ✅ 既存テストへの影響なし（失敗している 2 つのテスト `test_example_qsort_ws_self`, `test_scope_block_var_no_collision_001_ws_self` は元々失敗していたもの）

**結果**
- 全 3 フェーズの実装が完了
- 新規テストが全て成功
- 既存テストに新たな影響なし（元々失敗していたテストは 2 つ）
- 後方互換性を維持（`debug_ext=false` がデフォルト）
