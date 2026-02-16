# nospace → Whitespace コンパイラ

**実装状況**: ✅ 実装済み（`src/compiler_ws/` モジュール）

## ドキュメント移動のお知らせ

このディレクトリにあったドキュメントは、以下のように再整理されました：

### 旧実装の調査ドキュメント → `ai-docs/spec/compiler-legacy/`

旧実装（`.local/nospace/main.cpp`）の解析結果は、調査・仕様ドキュメントとして `ai-docs/spec/compiler-legacy/` に移動しました。

- [ai-docs/spec/compiler-legacy/README.md](../../spec/compiler-legacy/README.md) - インデックス
- overview.md, instructions.md, memory-layout.md など

### Rust実装設計ドキュメント → `ai-docs/spec/compiler-rust-impl/`

現在の nospace20 の Whitespace コンパイラ設計ドキュメントは `ai-docs/spec/compiler-rust-impl/` に移動しました。

- [ai-docs/spec/compiler-rust-impl/README.md](../../spec/compiler-rust-impl/README.md) - インデックス
- overview.md, whitespace.md, codegen.md など

### テスト戦略 → `ai-docs/spec/compiler-test-strategy.md`

コンパイラのテスト戦略は以下に移動しました：

- [ai-docs/spec/compiler-test-strategy.md](../../spec/compiler-test-strategy.md)

### 完了記録 → `ai-docs/done-task/`

環境構築の進捗記録は完了タスクとして移動しました：

- [ai-docs/done-task/ws-environment-setup-progress.md](../../done-task/ws-environment-setup-progress.md)
- [ai-docs/done-task/whitespace-integration-test.md](../../done-task/whitespace-integration-test.md)
- [ai-docs/done-task/phase4-implementation-report.md](../../done-task/phase4-implementation-report.md)

## 現在の実装

Whitespace コンパイラは `src/compiler_ws/` モジュールに実装されています。

### 実装済み機能

- ✅ 基本的な演算・制御構造
- ✅ 関数定義・呼び出し（main関数のみ）
- ✅ グローバル/ローカル変数
- ✅ 配列サポート（Phase 4）
- ✅ 参照・参照解除演算子
- ✅ 組み込みI/O関数

### 未実装機能

- ❌ ユーザー定義関数呼び出し
- ❌ break 文
- ❌ continue 文

詳細は [whitespace-missing-features.md](whitespace-missing-features.md) を参照してください。

### テスト

Whitespace コンパイラのテストは以下で実行できます：

```bash
# 統合テスト（wsc が必要）
cargo test --test compile_test -- --ignored

# wsc のセットアップ
./tools/setup-wsc.sh
```

## 今後のタスク

現在、以下の機能が未実装です：

1. **ユーザー定義関数呼び出し** (Phase 7)
   - 複数関数の定義生成
   - 関数呼び出しのコード生成
   
2. **break/continue 文** (Phase 8)
   - ループラベルスタックの実装
   - break/continue 文のコード生成

詳細は [whitespace-missing-features.md](whitespace-missing-features.md) を参照してください。

- 制御構造（if/elsif/else、while）
- I/O（整数・文字の入出力）

### 制限事項

- 論理演算子は短絡評価を行わない
- ネストした関数定義は不可
- break/continue は未実装

## 参考

- 旧実装ソース: `.local/nospace/main.cpp`
- Whitespace 仕様: `spec-whitespace.md`
- nospace 言語仕様: `spec.md`
