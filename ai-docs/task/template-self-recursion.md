# テンプレート関数の自己再帰サポート

テンプレート関数のボディ内で自分自身を再帰呼び出しできるようにする修正の設計。

最終更新日: 2026-03-04

## 1. 背景・問題

### 1.1 現状の動作

テンプレート関数は `expand_template_instantiations()` でインスタンス化される。
この処理では:

1. `TemplateFunctionDefinition` は出力ステートメントリストから除外される
2. `AliasInstantiation` は `FunctionDeclaration` に変換される（ボディをクローン）

テンプレートのボディ内でテンプレート名自身を呼び出すコードがある場合、クローン後もテンプレート名への参照がそのまま残る。しかしテンプレート定義自体は出力から除外されているため、後続の意味解析で「未定義の関数」エラーとなる。

### 1.2 再現例

```nospace
func: foo(i), alias: func: callback() {
  if: callback(i) {
    return: i;
  };
  return: foo(i-2);  # ← foo はテンプレート名。展開後は未定義となる
}
func: cb(x) {
  return: x < 5;
}

alias: myFoo(foo, cb);

func: __main() {
  __puti(myFoo(2));
}
```

`alias: myFoo(foo, cb)` により生成される関数は以下と同等:

```nospace
func: myFoo(i) {
  alias: callback(cb);   # 合成された alias
  if: callback(i) {
    return: i;
  };
  return: foo(i-2);      # ← foo は未定義 → エラー
}
```

### 1.3 期待される動作

`foo(i-2)` は同じインスタンス化 `myFoo(i-2)` として解決されるべき。

展開後に期待される結果:

```nospace
func: myFoo(i) {
  alias: callback(cb);   # 合成された alias
  alias: foo(myFoo);     # ← 自己参照 alias を追加
  if: callback(i) {
    return: i;
  };
  return: foo(i-2);      # → myFoo(i-2) に解決される
}
```

## 2. 設計

### 2.1 修正方針

テンプレートインスタンス化時に、**テンプレート名→インスタンス名** の `AliasIdentifier` を合成ボディの先頭に自動挿入する。

- 挿入位置: alias パラメータの合成文の後、テンプレートボディのクローンの前
- 既にテンプレート名が alias パラメータ名と衝突する場合: alias パラメータ名が優先される（後から挿入される alias パラメータの合成文は先に挿入される自己参照 alias を上書きする可能性はないが、name が alias パラメータ名と同じ場合は自己参照 alias は不要かつ意図しない影響を与えるため、名前衝突時はスキップする）

### 2.2 修正箇所

**ファイル**: `src/semantic_analyzer/template.rs`
**関数**: `expand_template_instantiations()`
**変更内容**: `AliasInstantiation` の展開処理で、alias パラメータの合成文を挿入した後、テンプレートボディのクローン前に以下を追加:

```rust
// 自己再帰サポート: テンプレート名→インスタンス名の alias を挿入
// テンプレート名が alias パラメータ名と衝突しない場合のみ
let conflicts_with_param = template.alias_params.iter().any(|p| p.name == *template_name);
if !conflicts_with_param {
    synthetic_body.push(LocatedStatement {
        statement: Statement::AliasIdentifier(
            template_name.clone(),
            name.clone(),
        ),
        location: loc.clone(),
    });
}
```

### 2.3 影響範囲

- **既存のテンプレート関数**: 影響なし。テンプレートボディ内で自分自身を参照しない限り、追加された alias は使われない。自分自身を参照するコードがなく、かつテンプレート名と同名の変数・関数がテンプレートの定義元スコープに存在する場合、その名前解決が変わる可能性はあるが、テンプレート名はスコープ内で一意であるため（テンプレート定義自身が同名で登録されていた）、この問題は起きない。
- **compiler_ws / interpreter**: 変更不要。展開後は通常の `FunctionDeclaration` として処理される。
- **tree_parser**: 変更不要。

### 2.4 テストケース

#### 成功テスト

| テスト名 | 内容 |
|---------|------|
| `template_func_self_recursion_001` | テンプレート関数内での自己再帰呼び出し（func alias + 自己再帰） |

#### エラーテスト

なし（自己再帰を許可する方向の変更）。

### 2.5 仕様反映

`docs/spec.md` のテンプレート関数セクション「テンプレートのルール」に以下を追加:

> - テンプレート関数のボディ内でテンプレート名を関数呼び出しすると、インスタンス化された関数への自己再帰呼び出しとなる。

## 3. 実装ステップ

| Step | 内容 | 状態 |
|------|------|------|
| 1 | `template.rs` の `expand_template_instantiations()` に自己参照 alias 挿入を追加 | 未着手 |
| 2 | テストケース追加 | 未着手 |
| 3 | `docs/spec.md` の仕様反映 | 未着手 |

## 関連ドキュメント

- [template-functions.md](template-functions.md) - テンプレート関数の設計全体
- [docs/spec.md](../../docs/spec.md) - 言語仕様
