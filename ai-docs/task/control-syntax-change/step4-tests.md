# Step 4: テストの修正

## 概要

Step 2・3 の構文変更により、既存のテストケースの `if` / `while` 構文を新構文に更新する必要がある。
ほぼ全てのテストに影響がある。

## 対象

### resources/tests/

nospace テストケースファイル（`.ns` ソースコードおよび `check.json`）。
`if` / `while` を使用する全テストが対象。

### resources/tests_ws/

Whitespace テストケース。nospace 構文を使用しないため、影響なし。

### tests/

Rust 統合テスト。テスト内でハードコードされた nospace コードがあれば更新が必要。

### tmp/

一時ファイル。テストに含まれないが、参考としてある `.ns` ファイルは必要に応じて更新。

## 変換パターン

### while

```
# 変更前
while: cond {
  body
};

# 変更後
while: cond, {
  body
};
```

パターン: `while:` と `{` の間に `,` を挿入。

### if（else なし）

```
# 変更前
if: cond {
  body
};

# 変更後
if: cond, {
  body
};
```

パターン: `if:` 条件式と `{` の間に `,` を挿入。

### if-else

```
# 変更前
if: cond {
  then_body
} else: {
  else_body
};

# 変更後
if: cond, {
  then_body
}, else: {
  else_body
};
```

パターン: 条件式と `{` の間に `,` を挿入、`}` と `else:` の間に `,` を挿入。

### if-else if（→ elif）

```
# 変更前
if: cond1 {
  body1
} else: if: cond2 {
  body2
} else: {
  body3
};

# 変更後（elif 使用）
if: cond1, {
  body1
}, elif: cond2, {
  body2
}, else: {
  body3
};
```

`else: if:` → `elif:` への変換は任意。`else: if:` も引き続き動作する。

## 方針

- 機械的な置換が可能な範囲はスクリプト・一括置換で対応
- 手動確認が必要なケースは個別対応
- 全テストが通ることを確認

## 作業順序

1. Step 2 完了後: `while` を使用するテストを更新
2. Step 3 完了後: `if` を使用するテストを更新
3. 全テスト実行・確認
