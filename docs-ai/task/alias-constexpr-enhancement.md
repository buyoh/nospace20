# テンプレート関数の constexpr パラメータに定数式を渡せるようにする

## 背景

テンプレート関数の `constexpr:` パラメータに渡せるのは現状「整数リテラルまたは識別子」のみ。
型システム導入後、`sizeof: StructName` のような定数式を直接渡したいケースが想定される。

```nospace
# 現状: 不可
alias: alloc_point(alloc_struct, sizeof: Point);

# 現状: 回避策
constexpr: POINT_SIZE(sizeof: Point);
alias: alloc_point(alloc_struct, POINT_SIZE);
```

## 提案

`alias:` のインスタンス化引数で `constexpr:` パラメータに対して定数式（`sizeof:` を含む）を直接渡せるようにする。

## ステータス

**未着手** — 型システム実装後にあらためて検討する。
