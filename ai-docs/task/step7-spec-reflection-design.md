# Step 7: spec.md / grammar.bnf への反映 設計メモ

## 概要

Step 2〜5 で実装された機能を言語仕様ドキュメントに反映する。

**親ドキュメント**: [unimplemented-variable-features.md](unimplemented-variable-features.md) §4.8

---

## 1. grammar.bnf への追加

### 1.1 現状

`docs/grammar.bnf` の「未実装機能」コメントに以下がある:
```
# - final / const 修飾子
```

constexpr と alias は文法定義に含まれていない。

### 1.2 追加する文法規則

```bnf
## constexpr 定義
constexpr_decl ::=
    | "constexpr" ":" constexpr_item ("," constexpr_item)* ";"

constexpr_item ::= ident "(" expr ")"

## alias 定義（識別子エイリアス）
alias_ident_decl ::=
    | "alias" ":" alias_ident_item ("," alias_ident_item)* ";"

alias_ident_item ::= ident "(" ident ")"

## alias 定義（ブロックエイリアス）（Step 4 実装後）
alias_block_decl ::=
    | "alias" ":" ident block ";"

## final 変数定義（Step 5 実装後）
final_decl ::=
    | "final" ":" let_decl ("," let_decl)* ";"
```

### 1.3 stmt 規則の更新

```bnf
stmt ::=
    | if_stmt
    | while_stmt
    | repeat_stmt
    | for_stmt
    | return_stmt
    | break_stmt
    | continue_stmt
    | let
    | static
    | final_decl                              # 追加
    | constexpr_decl                          # 追加
    | alias_ident_decl | alias_block_decl     # 追加
    | func
    | expr ";"
```

### 1.4 global_stmt 規則の更新

```bnf
global_stmt ::=
    | let
    | static
    | constexpr_decl                          # 追加
    | alias_ident_decl | alias_block_decl     # 追加
    | expr ";"
```

### 1.5 未実装機能コメントの更新

```bnf
## 未実装機能
#
# - constexpr ブロック形式 (constexpr: name { ... };)
```

`final / const 修飾子` の項目を削除し、実装済みの constexpr / alias / final を正式な文法として記載。

---

## 2. spec.md への追加

### 2.1 変数定義セクションの更新

「代入・変数定義」セクション（~L175-190）の未実装コメントを更新:

**現状**:
```markdown
- (未実装) `final` 再代入不可。`const` リテラルのみ代入可かつ再代入不可。
- (未実装) `const` の参照を取得することはできない。コンパイル時定数と同等の扱い。
```

**更新後**（Step 5 実装後）:
```markdown
- `final` 再代入不可。初期値設定後は代入できない。`&final_var` でアドレス取得は可能。
- `constexpr` コンパイル時定数。スタックスロットを確保せず、コンパイル時に値に解決される。`&constexpr_var` は不可。
```

### 2.2 constexpr セクションの追加

変数定義セクションの後に constexpr の仕様を追加。

```markdown
### constexpr 定義

コンパイル時に評価される定数を定義する。スタックスロットを確保しない。

\```nospace
constexpr: PI(3);
constexpr: SIZE(2 + 3);           # = 5 #
constexpr: DOUBLE_PI(PI * 2);     # 他の constexpr を参照可能 #
# constexpr: X(variable);         # コンパイルエラー: 非定数式 #
\```

- コンパイル時に評価可能な式のみ使用可能（リテラル、他の constexpr、算術/比較/論理演算）
- ホイスティングされる（スコープ内で定義位置より前に使用可能）
- 巡回参照はコンパイルエラー
```

### 2.3 alias セクションの更新

現在の `docs/spec.md` の「alias 定義」セクション（~L287-320）は
テンプレート関数（関数パラメータとしての alias）の仕様が記載されている。
これとは別に、識別子エイリアスとブロックエイリアスの仕様を追加する。

```markdown
### alias（識別子エイリアス）

既存の識別子（変数名・関数名）に別名を付ける。コンパイル時に名前が置換される。

\```nospace
func: func1() { return: 42; }
alias: afunc(func1);         # afunc は func1 の別名 #
\```

### alias（ブロックエイリアス）（Step 4 実装後に追加）

ブロック（文の列）を名前に紐付け、呼び出し時に AST をインライン展開する。

\```nospace
alias: greet {
  __puti(42);
};
greet();    # ブロックが展開される #
\```
```

### 2.4 final セクションの追加（Step 5 実装後）

```markdown
### final 変数

再代入不可の変数を定義する。初期値設定後は代入できない。

\```nospace
final: x(10);
# x = 20;      # コンパイルエラー: 再代入不可 #
&x;             # アドレス取得は可能 #
\```
```

---

## 3. 実装タイミング

Step 7 は Step 2〜5 の各実装完了に合わせて段階的に反映する:

| 反映対象 | 前提 | 状態 |
|---------|------|------|
| constexpr（式形式） | Step 2 ✅ | 反映可能 |
| alias（識別子エイリアス） | Step 3 ✅ | 反映可能 |
| alias（ブロックエイリアス） | Step 4 | Step 4 実装後 |
| final 変数 | Step 5 | Step 5 実装後 |

**推奨**: Step 4 と Step 5 の実装が完了した後にまとめて反映する。
ただし、既に実装済みの constexpr と識別子エイリアスは先行して反映しても良い。

---

## 5. 進捗

### 2026-03-01 完了

Step 2〜6 の実装がすべて完了したため、ドキュメントの反映を実施した。

#### 実施内容

- `docs/grammar.bnf`:
  - `final`, `constexpr`（式形式・ブロック形式）, `alias`（識別子・ブロック）の文法規則を追加
  - `global_stmt` と `stmt` に `final`, `constexpr`, `alias` を追加
  - 未実装機能コメント「final / const 修飾子」を削除
  - constexpr ブロック形式はブロック形式として文法定義済み

- `docs/spec.md`:
  - 「代入・変数定義」セクションの `final`/`const` 未実装コメントを実装済み説明に更新
  - `constexpr`、`constexpr ブロック形式`、`final 変数` セクションを追加
  - `alias 定義`（識別子エイリアス・ブロックエイリアス）セクションを追加

#### テスト結果

全テスト通過:
- 63 constexpr テスト通過
- 21 block-alias テスト通過
- 14 final 変数テスト通過
- 6 constexpr コンパイルエラーテスト通過
- 2 block-alias コンパイルエラーテスト通過
- 3 final 変数コンパイルエラーテスト通過

---

## 4. 変更ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `docs/grammar.bnf` | constexpr / alias / final の文法規則追加、未実装コメント更新 |
| `docs/spec.md` | constexpr / alias / final の仕様セクション追加・更新 |
