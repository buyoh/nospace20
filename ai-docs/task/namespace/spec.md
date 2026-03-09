# 名前空間: 言語仕様

## 構文

```bnf
namespace_decl ::= "namespace" ":" ident "{" (stmt)* "}"
```

`namespace:` はキーワード＋コロンの形式で、他のキーワードと一貫する。
ブロック末尾にセミコロンは**不要**（`func:` と同様の扱い）。

```
namespace: MySpace {
  let: x(2);
  func: helper() { return: 42; }
}
```

### ドットアクセス

名前空間内の識別子へのアクセスは `.`（ドット）演算子を使用する。

```bnf
qualified_ident ::= ident ("." ident)*
```

```
let: y;
y = MySpace.x;
MySpace.helper();
```

ドットは識別子の一部として扱い（修飾識別子）、演算子としてではない。
これにより、式パーサへの変更を最小限に抑える。

## セマンティクス

### 名前空間はスコープではない

名前空間はスコープを作成**しない**。変数のライフタイムは、名前空間ブロックを含む外側のスコープ（関数スコープやブロックスコープ）に依存する。

```
func: __main() {
  let: x(1);
  namespace: MySpace {
    let: x(2);           # 外側スコープに MySpace.x として定義 #
    namespace: MySpace2 {
      let: x(3);         # 外側スコープに MySpace.MySpace2.x として定義 #
    }
    __clog(x);            # 2 を出力。MySpace.x に解決 #
    __clog(MySpace2.x);   # 3 を出力。MySpace.MySpace2.x に解決 #
  }
  __clog(x);              # 1 を出力。外側の x #
  __clog(MySpace.x);      # 2 を出力 #
  __clog(MySpace.MySpace2.x);  # 3 を出力 #
  # MySpace2.x;           コンパイルエラー: MySpace2 はこのスコープに存在しない #
}
```

- namespace ブロック内で宣言された変数は、外側のスコープのスロットを使用する
- ホイスティングは外側スコープの規則に従う
- 変数の破棄は外側スコープが終了するとき

### 名前解決ルール

名前空間ブロック内での識別子解決は、以下の優先順位で行われる。

1. **現在の名前空間プレフィックスで修飾した名前**を探索
   - `namespace: A { x; }` → まず `A.x` を探す
2. **見つからない場合、プレフィックスなしの名前**で通常のスコープ解決を行う
   - `namespace: A { y; }` → `A.y` が無ければ外側の `y` を探す

修飾名（ドット付き）は常に絶対的に解決される。

```
let: x(1);
namespace: A {
  let: x(2);
  __clog(x);     # A.x = 2（名前空間内の x が優先）#
}
__clog(x);       # 1（外側の x）#
__clog(A.x);     # 2（修飾名で参照）#
```

### 名前空間内に配置可能な宣言

| 宣言 | 対応 | 備考 |
|------|------|------|
| `let:` / `static:` / `final:` | 可 | 変数名にプレフィックスが付加される |
| `func:` | 可 | 関数名にプレフィックスが付加される |
| `constexpr:` | 可 | 定数名にプレフィックスが付加される |
| `alias:` | 可 | エイリアス名にプレフィックスが付加される |
| `namespace:` | 可 | ネスト可能。プレフィックスが連結される |
| `if:` / `while:` / `for:` / `repeat:` | 不可 | 制御構文は名前空間内に直接配置不可 |
| 式文 | 可 | 名前空間内で式を評価可能（副作用のある初期化等） |

### ネスト

名前空間はネスト可能。内側の名前空間のプレフィックスは外側と連結される。

```
namespace: Outer {
  namespace: Inner {
    let: val(42);
    # val は Outer.Inner.val としてマングルされる #
  }
  __clog(Inner.val);  # 解決: Outer.Inner.val #
}
__clog(Outer.Inner.val);  # 解決: Outer.Inner.val #
```

### 名前空間の再オープン

同一スコープ内で同名の名前空間を複数回宣言することは**許可しない**。

```
namespace: A { let: x(1); }
namespace: A { let: y(2); }  # コンパイルエラー: namespace 'A' is already defined #
```

理由: 名前空間がスコープではないため、再オープンのセマンティクスが曖昧になる。同一名前空間に追加したい場合は、一つのブロックにまとめる。

### `__main` との関係

`__main` 関数はグローバルスコープのエントリーポイントとして特別扱いされる。名前空間内の `__main` は通常の名前空間付き関数（例: `MySpace.__main`）となり、エントリーポイントにはならない。

```
namespace: Module {
  func: __main() { return: 0; }
  # → Module.__main として登録。エントリーポイントではない #
}
func: __main() {
  Module.__main();  # 通常の関数呼び出し #
}
```

### 組み込み関数との関係

組み込み関数 (`__clog`, `__puti` 等) は名前空間プレフィックスの影響を受けない。
名前解決時、エイリアスチェーンが解決された後の最終名が `__` で始まる場合は組み込み関数として扱われる。

```
namespace: Util {
  __clog(42);  # 組み込み関数。Util.__clog ではない #
}
```

### ホイスティング

名前空間宣言自体はホイスティング**されない**。名前空間ブロック内の宣言（変数・関数）は通常通りホイスティングされるが、その影響範囲は外側のスコープ全体ではなく、**名前空間ブロックの意味解析時**に限定される。

ただし、名前空間内で定義された関数は、名前空間外からマングル名で参照できるようにホイスティングされる（関数ホイスティングの規則に従う）。

```
func: __main() {
  A.f();  # OK: A.f は関数ホイスティングにより利用可能 #
  namespace: A {
    func: f() { __clog(1); }
  }
}
```

### セミコロン

名前空間の末尾にセミコロンは不要（`func:` と同様）。

```
namespace: A {
  let: x(1);
}             # ← セミコロン不要 #
```

## BNF 追加

```bnf
# Statements (追加)
namespace_stmt ::= "namespace" ":" ident "{" (stmt)* "}"

# global_stmt に追加
global_stmt ::= ... | namespace_stmt

# stmt (関数内) に追加
stmt ::= ... | namespace_stmt

# Tokens (追加)
dot ::= "."

# ident を拡張（修飾識別子）
qualified_ident ::= ident ("." ident)*
```

## エラーケース

| エラー | メッセージ例 |
|--------|-------------|
| 同名の名前空間が既に定義済み | `namespace 'A' is already defined in this scope` |
| 存在しない名前空間への修飾アクセス | `namespace 'A' is not defined` |
| 制御構文の直接配置 | `control flow statements are not allowed directly inside namespace` |
| 名前空間名と変数・関数名の衝突 | `'A' is already defined as a variable/function` |
