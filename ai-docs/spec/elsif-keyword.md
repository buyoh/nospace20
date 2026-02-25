# elsif キーワード仕様

## 構文

```
if: 条件式 { 文... } elsif: 条件式 { 文... } else: { 文... };
```

### BNF

```bnf
if_stmt ::=
    | "if" ":" expr block elsif_chain? ";"

elsif_chain ::=
    | "elsif" ":" expr block elsif_chain?
    | "else" ":" block
```

## セマンティクス

- `elsif:` は `else: if:` の糖衣構文ではなく、独立したキーワード
- `else: if:` 構文は廃止（パースエラー）
- AST 上は入れ子の `Expression::If` として表現される（`else: if:` と同じ構造）
- 意味解析・実行の振る舞いは従来と同一

## トークン

| トークン | 型 |
|----------|-----|
| `elsif` | `Keyword::Elsif` |

予約語リストに追加: `let`, `func`, `if`, `else`, **`elsif`**, `while`, `return`, `break`, `continue`, `static`

## 例

### 基本

```
if: x == 1 {
  __clog(1);
} elsif: x == 2 {
  __clog(2);
} else: {
  __clog(0);
};
```

### チェーン

```
if: x == 1 {
  __clog(1);
} elsif: x == 2 {
  __clog(2);
} elsif: x == 3 {
  __clog(3);
} else: {
  __clog(0);
};
```

### else なし

```
if: x == 1 {
  __clog(1);
} elsif: x == 2 {
  __clog(2);
};
```

### 式としての使用

```
let: r;
r = if: x == 1 { 10; } elsif: x == 2 { 20; } else: { 0; };
```

## 廃止: `else: if:` 構文

`else:` の直後に `if:` が来る構文はパースエラーとなる。`elsif:` を使用すること。

```
# エラー
if: x { } else: if: y { };

# 正しい
if: x { } elsif: y { };
```
