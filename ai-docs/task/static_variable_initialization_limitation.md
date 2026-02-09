# static 変数の未実装問題

## 概要

複数変数宣言・初期化宣言の実装時に、**static 変数の基本機能自体が動作していない** ことが判明した。

## 問題

`static:` で宣言された変数が、関数呼び出しをまたいで値を保持しない。

### テストコード

```nospace
func: setter() {
  static: static_var;
  static_var = static_var + 1;
  __clog(static_var);
}

func: main() {
  setter();  # 期待: 1 #
  setter();  # 期待: 2 #
  setter();  # 期待: 3 #
}
```

### 実際の出力

```
__clog: 1
__clog: 1
__clog: 1
```

毎回 1 が出力される。static_var が保持されていない。

## 仕様上の動作

spec.md §7 より:

```
func: setter() {
  static: static_var;  # static変数 #
  static_var += 1;
  __clog(static_var);  # 呼び出される度に 1, 2, 3, ... と増加 #
  global_var = static_var;  # グローバル変数に代入 #
}
```

static 変数は:
- グローバルスコープの変数と同じタイミングで初期化される
- 関数が呼び出されても再初期化されない
- 関数呼び出しをまたいで値を保持する

## 既存テストの状況

`resources/tests/test-manifest.yaml` を確認すると、**すべての static 変数テストがコメントアウト** されている:

```yaml
# - name: test_scope_scope_static_001
#   type: success
#   path: scope/scope_static_001
#   comment: "Static variable basic functionality"

# - name: test_scope_scope_static_nested_001
# - name: test_scope_scope_static_mixed_001
# - name: test_scope_scope_static_multi_decl_001
# - name: test_scope_scope_static_counter_factory_001
```

つまり、**static 変数の機能は既知の未実装項目** である。

## 本タスクとの関係

本タスク (複数変数宣言・初期化宣言) では:
- `let: a, b;` の複数宣言をサポート → ✅ 完了
- `let: x(5);` の初期化をサポート → ✅ 完了
- `static: a, b;` の複数宣言をサポート → ✅ 構文的には完了

しかし、`static:` の初期化式 `static: count(0);` をテストしようとしたところ、**static 変数自体が動作していない** ことが判明した。

## 対応

### 本タスクでの対応

- `test_variables_var_static_init` を削除
- `static:` の複数宣言構文は実装されたが、動作確認はできない
- ドキュメントに制限事項を記録

### 今後の対応

static 変数の実装が必要。以下のいずれかの問題がある可能性:

1. **セマンティック解析**: static フラグが正しく処理されていない
2. **インタプリタ**: static 変数の格納場所が間違っている（ローカルスタックに配置されている？）
3. **変数管理**: 関数スコープごとに変数が再作成されている

調査が必要なコード:
- `src/semantic_analyzer/mod.rs`: static フラグの処理
- `src/interpreter/exec.rs`: 変数の格納場所の決定
- `src/interpreter/mod.rs`: グローバル変数の初期化

## 参照

- 本タスク: `ai-docs/task/implement-multi-variable-declaration.md`
- 関連する未実装機能: spec.md §7 スコープ
- コメントアウトされたテスト: `resources/tests/passes/scope/scope_static_*.ns`
