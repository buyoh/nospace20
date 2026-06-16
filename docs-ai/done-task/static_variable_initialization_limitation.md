# static 変数の未実装問題 → 解決済み

## 結論

**2026-02-10 の調査の結果、static 変数の基本機能は既に正しく動作していることを確認。追加の実装は不要。**

## 元の問題報告

複数変数宣言・初期化宣言の実装時に、static 変数の基本機能が動作していないという報告があった。

## 調査結果

### 動作確認

以下のテストコードで正しい出力を確認:

```nospace
func: setter() {
  static: static_var;
  static_var = static_var + 1;
  __clog(static_var);
}

func: main() {
  setter();
  setter();
  setter();
}
```

出力: `__clog: 1`, `__clog: 2`, `__clog: 3` (期待通り)

初期化式付きも正常動作:

```nospace
func: setter() {
  static: count(10);
  count = count + 1;
  __clog(count);
}
```

出力: `__clog: 11`, `__clog: 12`, `__clog: 13` (期待通り)

### テスト状況

以下の4つの static 関連テストが有効化されており、全てパス:

- `test_scope_scope_static_001` - 基本的な static 変数
- `test_scope_scope_static_init_001` - ルートレベル static 変数の初期化
- `test_scope_scope_static_init_order_001` - static 変数の初期化順序
- `test_scope_scope_static_persist_001` - 関数ローカル static 変数の永続化

コメントアウトされているテストは **ネスト関数サポート (Phase 5)** が必要なもののみ:

- `scope_static_nested_001` - ネスト関数からの static 変数アクセス
- `scope_static_mixed_001` - static/非static 混在（ネスト関数）
- `scope_static_multi_decl_001` - 複数 static 変数宣言（ネスト関数）
- `scope_static_counter_factory_001` - カウンターファクトリパターン（ネスト関数）
- `scope_static_error_001` - 非static 変数のスコープ越え参照エラー（ネスト関数）

### 実装の確認

static 変数の永続化は以下のメカニズムで正しく実装されている:

1. **初期化** (`interpreter/mod.rs`): `initialize_function_statics` が全関数をスキャンし、static 変数を持つ関数の永続ストレージを作成
2. **復元** (`interpreter/exec.rs`): 関数呼び出し時に `function_static_storage` から static 変数の値を復元
3. **保存** (`interpreter/exec.rs`): 関数終了時にスコープデータを `function_static_storage` に保存

## 元の問題の原因推測

この文書が作成された時点では Phase 4 (static 変数の永続化) が未実装だった可能性がある。その後の `scope-phase4-static-variables` タスクで実装が完了し、問題は解決された。
- `src/interpreter/mod.rs`: グローバル変数の初期化

## 参照

- 本タスク: `docs-ai/task/implement-multi-variable-declaration.md`
- 関連する未実装機能: docs/spec.md §7 スコープ
- コメントアウトされたテスト: `resources/tests/passes/scope/scope_static_*.ns`
