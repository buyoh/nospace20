# DONE: 複合代入演算子の実装

## 概要

`test_legacy_023` が失敗している。`+=`, `-=`, `*=`, `/=`, `%=` という複合代入演算子がパーサーでサポートされていないため。

## 実装完了

### 実装内容

1. **トークナイザー** (`src/token_parser/mod.rs`)
   - 新しいトークン型を追加: `PlusEqual`, `MinusEqual`, `AsteriskEqual`, `SlashEqual`, `PercentEqual`
   - `+`, `-`, `*`, `/`, `%` のパース処理を修正し、後続に `=` があるかチェック

2. **パーサー** (`src/tree_parser/expression/mod.rs`)
   - `Operator2` enum に複合代入演算子を追加: `PlusAssign`, `MinusAssign`, `MultiplyAssign`, `DivideAssign`, `ModuloAssign`
   - `parse_to_expression_tree_assign` で複合代入トークンを認識

3. **セマンティック解析** (`src/semantic_analyzer/mod.rs`)
   - `convert_to_exec_expression_with_resolver` で複合代入演算子を `a = a + b` 形式に展開
   - 例: `a += b` → `a = a + b`

4. **インタプリタ** (`src/interpreter/exec.rs`)
   - 複合代入演算子のパターンマッチに `unreachable!()` を追加（セマンティック解析で展開されるため）

5. **コンパイラ** (`src/compiler_ws/expression.rs`)
   - 複合代入演算子のパターンマッチに `unreachable!()` を追加（セマンティック解析で展開されるため）

### テスト結果

- `test_legacy_023`: ✅ 成功
- 全テスト: ✅ 93 passed; 0 failed; 14 ignored

### コミット

- コミットID: 0397e73
- メッセージ: "feat: implement compound assignment operators (+=, -=, *=, /=, %=)"

## 仕様との整合性

✅ 複合代入演算子は式として値を返す（代入後の値）
✅ 右結合 (`x += y += 3` は `y += 3; x += y;` と等価)
✅ docs/spec.md の記載通りに動作

## 問題の詳細

### エラー内容

```
error: unexpected token
  (internal: src/tree_parser/expression/mod.rs:83)
line:5 column:14
    __puti(a += b);
              ^
```

### 原因

- パーサーが複合代入演算子をサポートしていない
- トークナイザーが `+=` などをトークンとして認識していない可能性

### 仕様

docs/spec.md の記載:

```
x += 1;   # x = x + 1 と等価 #
```

```
x += y += 3;  # y += 3; x += y; と等価 #
```

また、`docs/grammar.bnf` にも以下の記載あり:

```
# - 複合代入演算子 (+=, -=, *=, /=, %=)
```

### テストケース

`resources/tests/passes/legacy/legacy_023.ns`:

```nospace
func:main(){
    let:a,b;
    a = 3; b = 4;
    __puti(a + b);
    __puti(a += b);
    __puti(a);
    __putc(';');
    a = 7; b = 3;
    __puti(a - b);
    __puti(a -= b);
    __puti(a);
    __putc(';');
    a = 3; b = 2;
    __puti(a * b);
    __puti(a *= b);
    __puti(a);
    __putc(';');
    a = 10; b = 2;
    __puti(a / b);
    __puti(a /= b);
    __puti(a);
    __putc(';');
    a = 12; b = 5;
    __puti(a % b);
    __puti(a %= b);
    __puti(a);
    __putc(';');
}
```

## 実装に必要な作業

### トークナイザー側

1. `src/token_parser/mod.rs` で複合代入演算子をトークンとして認識
   - `+=`, `-=`, `*=`, `/=`, `%=` を新しいトークンとして追加
   - `+` と `=` を別々に認識するのではなく、`+=` として認識する必要がある

### パーサー側

1. `src/tree_parser/expression/mod.rs` で複合代入演算子のパース処理を実装
   - 代入式のパース処理を修正
   - `=` だけでなく `+=`, `-=` などもサポート

2. AST (抽象構文木) の修正
   - `Expression::Assign` に演算子の種類を追加
   - または `Expression::CompoundAssign { op, lhs, rhs }` のような新しいノードを追加

### セマンティック解析側

1. 複合代入演算子を処理できるように修正
   - `a += b` を `a = a + b` に展開

### インタプリタ/コンパイラ側

1. 複合代入演算子の実行処理を実装
   - インタプリタ: 左辺の値を読み取り、演算を行い、結果を代入
   - コンパイラ: Whitespace コードを生成

## 優先度

中 - docs/spec.md に記載されているが、現在は未実装

## 備考

- 複合代入演算子は式としても使用できる（値を返す）
  - `__puti(a += b)` のように、代入後の値を返す
- 仕様によれば右結合である
  - `x += y += 3` は `y += 3; x += y;` と等価
