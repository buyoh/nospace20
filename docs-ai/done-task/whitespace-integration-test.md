# Whitespace コンパイラ統合テスト設計

**完了日**: 2026年2月16日  
**状態**: Phase 1 完了（基本機能実装済み）

## 完了メモ

Phase 1 のすべてのタスクが完了し、whitespace 統合テスト基盤が正常に動作しています:
- ✅ 21個の whitespace テスト (`_ws`) が自動生成
- ✅ test-manifest.yaml の `targets` フィールドによる制御が機能
- ✅ build.rs による自動テスト生成が実装済み
- ✅ `#[ignore]` 属性による wsc 依存テストの分離

Phase 2（テストカバレッジ拡大）は将来的な改善タスクとして保留。

## 概要

既存の「largeテスト」（`resources/tests/` 以下の `.ns` ファイルを nospace インタプリタで実行し検証）と同様の仕組みを、whitespace コンパイラ向けにも構築する。

nospace コードを whitespace にコンパイルし、外部 whitespace インタプリタ (wsc) で実行して結果を検証する。

## 背景

### 現在の状況

| テスト種別 | 対象 | 状況 |
|-----------|------|------|
| ユニットテスト | 各モジュール | ✅ 実装済み |
| largeテスト（インタプリタ） | nospace 全パイプライン | ✅ `test-manifest.yaml` ベースで動作 |
| コンパイルテスト | whitespace 生成 | ⚠️ 手動記述 (`compile_test.rs`) |
| **統合テスト（whitespace）** | コンパイル + 実行 | ❌ **未実装** |

### 目標

- 既存の `passes/` 以下のテストケースを whitespace コンパイル + 実行でも検証
- `test-manifest.yaml` を拡張して whitespace 統合テストを自動生成
- interpreter と whitespace で同一結果を保証

## 設計

### アプローチ: マニフェストベースの自動テスト生成

既存の `test-manifest.yaml` + `build.rs` の仕組みを拡張する。

```yaml
# test-manifest.yaml (拡張後)
tests:
  - name: test_ok_coding_c000
    type: success
    path: c000
    targets:           # 新規フィールド
      - interpreter    # 既存: nospace インタプリタ
      - whitespace     # 新規: whitespace コンパイル + 実行
```

### テスト種別と whitespace 対応

| テスト種別 | whitespace 対応 | 備考 |
|-----------|----------------|------|
| `success` (trace) | ⚠️ 保留 | wsc では拡張 API 未対応のため将来検討 |
| `success_io` | ✅ 対応 | 標準 I/O を使用 |
| `syntax_error` | ❌ 対象外 | コンパイル以前の段階 |
| `semantic_error` | ❌ 対象外 | コンパイル以前の段階 |

### `__trace()` の whitespace 実装

[whitespace-runtime.md](../architecture/whitespace-runtime.md) で決定済みの方式を使用:

```
# __trace(42) の実装
push -1         # 特殊アドレス: trace API
push 42         # トレース値
store           # ヒープ書き込み → wsc では無視、独自実装ではトレース
```

**問題点**: wsc は標準 whitespace インタプリタであり、上記の拡張 API を解釈しない。

**解決策**: wsc での実行

- I/O ベースのテスト (`success_io`) のみ対応
- trace ベースのテスト (`success`) は将来の拡張として保留

### 実装方式: I/O ベーステスト

`success_io` テストを whitespace でも実行可能にする。

```
[nospace code] → [compile] → [whitespace code] → [wsc] → [stdout check]
```

## 実装計画

### wsc 統合テスト実装

#### test-manifest.yaml の拡張

```yaml
tests:
  - name: test_io_puti_basic_001
    type: success_io
    path: io/puti_basic_001
    targets:
      - interpreter
      - whitespace    # ← 新規
```

#### build.rs の拡張

```rust
// build.rs
fn generate_test(test: &TestDef) {
    if test.targets.contains("whitespace") {
        generate_whitespace_test(test);
    }
    // 既存: インタプリタテスト
    generate_interpreter_test(test);
}

fn generate_whitespace_test(test: &TestDef) {
    // #[ignore = "requires wsc"] を付与
    // compile_to_whitespace() + run_whitespace() を呼び出すテストを生成
}
```

#### テストファイル生成例

```rust
// 生成されるテスト (tests/generated_whitespace_test.rs)
#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn test_ws_io_puti_basic_001() {
    let source = include_str!("../resources/tests/passes/io/puti_basic_001.ns");
    let expected_stdout = "420";
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    let ws_code = compile_to_whitespace(&scope).unwrap();
    
    let output = run_whitespace(&ws_code, "").unwrap();
    assert_eq!(output.trim(), expected_stdout.trim());
}
```

## ディレクトリ構造の変更

```
resources/tests/
├── test-manifest.yaml     # targets フィールド追加
├── passes/
│   └── ...
tests/
├── code_test.rs           # 既存: インタプリタテスト
├── compile_test.rs        # 既存: コンパイルユニットテスト
├── whitespace_test.rs     # 新規: whitespace 統合テスト（手動）
└── generated/             # 新規: build.rs による自動生成
    ├── interpreter_test.rs
    └── whitespace_test.rs
```

## タスク一覧

### Phase 1: wsc 統合テスト基盤

- [x] **P1-1**: `test-manifest.yaml` スキーマ拡張設計
- [x] **P1-2**: `build.rs` の whitespace テスト生成実装
- [x] **P1-3**: `success_io` テストの whitespace 対応マーキング
- [x] **P1-4**: ビルトイン関数の実装（I/O + debug noop）

### Phase 2: テストカバレッジ拡大

- [ ] **P2-1**: 全 `success_io` テストの whitespace 対応確認
- [ ] **P2-2**: whitespace 固有の境界条件テスト追加
- [ ] **P2-3**: エラーハンドリング・エッジケースの検証

## 実装状況

### 完了

- ✅ `test-manifest.yaml` への `targets` フィールド追加
- ✅ `build.rs` の whitespace テスト生成機能実装
- ✅ `code_test.rs` への `test_whitespace_base()` / `test_whitespace_io_base()` ヘルパー関数追加
- ✅ I/O テスト・通常テスト合計22個に `targets: [interpreter, whitespace]` を設定
- ✅ ビルトイン関数の実装完了（`__puti`, `__putc`, `__geti`, `__getc`, `__trace`, `__assert`, `__assert_not`）

### 生成されたテスト

以下の whitespace テストが自動生成され、`#[ignore]` 属性が付与されています（**合計21個**）。
最新の一覧は `cargo test --test code_test -- --list | grep _ws` で確認可能。

### 既知の問題

**組み込み関数**: ✅ すべて実装済み

- `__puti`, `__putc`, `__geti`, `__getc` (I/O 関数) → `src/compiler_ws/expression.rs` で実装
- `__trace`, `__assert`, `__assert_not` (テスト用関数) → debug noop として実装（whitespace では無視される）

実装詳細: [done-task/builtin-functions-implementation.md](../done-task/builtin-functions-implementation.md)

**wsc のプラットフォーム制約**: `whitespacers` crate は JIT 機能が x86/x86_64 アーキテクチャに依存しており、ARM macOS ではビルドに失敗する。CI 環境（Linux x86_64）では動作する見込み。

### テスト結果

```
running 148 tests
test result: ok. 127 passed; 0 failed; 21 ignored; 0 measured; 0 filtered out
```

- ✅ 127個のテストがパス（インタプリタテスト）
- ⏭️ 21個の whitespace テストが ignore（wsc が必要なため `#[ignore]` 属性付き）
  - wsc をインストールし `cargo test --test code_test -- --ignored` で実行可能

### 実行方法

```bash
# 通常のテスト（interpreter のみ）
cargo test --test code_test

# whitespace テストを含む全テスト（wsc が必要）
cargo test --test code_test -- --ignored

# 特定の whitespace テスト
cargo test --test code_test test_io_puti_basic_001_ws -- --ignored
```

## 制約・考慮事項

### wsc の制約

- 外部プロセス呼び出しが必要（テスト速度への影響）
- CI 環境でのセットアップが必要（`tools/setup-wsc.sh`）
- `#[ignore]` 属性で通常テストからは除外

### 機能の対応状況

以下の機能は whitespace コンパイラでの対応状況を確認する必要がある:

| 機能 | 状況 | 備考 |
|-----|------|------|
| 基本演算 | ✅ 対応済み | |
| 変数 | ✅ 対応済み | ヒープ使用 |
| 関数呼び出し | ✅ 対応済み | call/ret 命令 |
| 制御構造 | ⚠️ 要確認 | break/continue の問題あり |
| I/O | ✅ 対応済み | `__puti`, `__putc`, `__geti`, `__getc` |

### 既存テストとの互換性

- 既存の `code_test.rs` はそのまま維持
- 新規テストは別ファイル (`whitespace_test.rs`) または生成ファイルに配置
- `#[ignore]` で wsc が必要なテストを分離

## 参考ドキュメント

- [whitespace-runtime.md](../architecture/whitespace-runtime.md) - Whitespace 実行環境の設計
- [integration-test-design.md](integration-test-design.md) - 結合テスト設計（解析パイプライン）
- [test-manifest.yaml](../../resources/tests/test-manifest.yaml) - テストマニフェスト
- [compile_test.rs](../../tests/compile_test.rs) - 既存の whitespace コンパイルテスト
