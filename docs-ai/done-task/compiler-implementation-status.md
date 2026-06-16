# nospace → Whitespace コンパイラ

**実装状況**: ✅ 実装済み（`src/compiler_ws/` モジュール）

## ドキュメント移動のお知らせ

このディレクトリにあったドキュメントは、以下のように再整理されました：

### 旧実装の調査ドキュメント → `docs-ai/spec/compiler-legacy/`

旧実装（`.local/nospace/main.cpp`）の解析結果は、調査・仕様ドキュメントとして `docs-ai/spec/compiler-legacy/` に移動しました。

- [docs-ai/spec/compiler-legacy/README.md](../../spec/compiler-legacy/README.md) - インデックス
- overview.md, instructions.md, memory-layout.md など

### Rust実装設計ドキュメント → `docs-ai/spec/compiler-rust-impl/`

現在の nospace20 の Whitespace コンパイラ設計ドキュメントは `docs-ai/spec/compiler-rust-impl/` に移動しました。

- [docs-ai/spec/compiler-rust-impl/README.md](../../spec/compiler-rust-impl/README.md) - インデックス
- overview.md, whitespace.md, codegen.md など

### テスト戦略 → `docs-ai/spec/compiler-test-strategy.md`

コンパイラのテスト戦略は以下に移動しました：

- [docs-ai/spec/compiler-test-strategy.md](../../spec/compiler-test-strategy.md)

### 完了記録 → `docs-ai/done-task/`

環境構築・機能実装の進捗記録は完了タスクとして移動しました：

- [docs-ai/done-task/ws-environment-setup-progress.md](../../done-task/ws-environment-setup-progress.md)
- [docs-ai/done-task/whitespace-integration-test.md](../../done-task/whitespace-integration-test.md)
- [docs-ai/done-task/phase4-implementation-report.md](../../done-task/phase4-implementation-report.md)
- [docs-ai/done-task/compiler-whitespace-missing-features-implementation.md](../../done-task/compiler-whitespace-missing-features-implementation.md)

## 現在の実装

Whitespace コンパイラは `src/compiler_ws/` モジュールに実装されています。

### 実装済み機能

- ✅ 基本的な演算・制御構造
- ✅ 関数定義・呼び出し（ユーザー定義関数を含む）
- ✅ グローバル/ローカル変数
- ✅ 配列サポート
- ✅ 参照・参照解除演算子
- ✅ 組み込みI/O関数
- ✅ break/continue 文

### 制限事項

- 論理演算子は短絡評価を行わない
- ネストした関数定義は不可

詳細は [compiler-whitespace-missing-features-implementation.md](../../done-task/compiler-whitespace-missing-features-implementation.md) を参照してください。

### テスト

Whitespace コンパイラのテストは以下で実行できます：

```bash
# 統合テスト（wsc が必要）
cargo test --test compile_test -- --ignored

# wsc のセットアップ
./tools/setup-wsc.sh
```

## 参考

- 旧実装ソース: `.local/nospace/main.cpp`
- Whitespace 仕様: `docs/spec-whitespace.md`
- nospace 言語仕様: `docs/spec.md`
