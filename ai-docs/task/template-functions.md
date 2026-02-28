# テンプレート関数 (Template Functions)

関数ポインタの代替として、コンパイル時にパラメータを具体的な関数・定数・変数に置換するテンプレート関数機能の設計。

最終更新日: 2026-02-28

## 目次

1. [背景・動機](#1-背景動機)
2. [構文設計](#2-構文設計)
3. [セマンティクス](#3-セマンティクス)
4. [既存機能との関係](#4-既存機能との関係)
5. [実装計画](#5-実装計画)
6. [設計上の未決定事項](#6-設計上の未決定事項)

---

## 1. 背景・動機

### 1.1 問題: 関数ポインタが実装困難

nospace は Whitespace にコンパイルされるが、Whitespace のアーキテクチャ上、関数ポインタの実装が非常に困難である。

- Whitespace のサブルーチン呼び出し (`call`) はラベル（静的な値）を指定する必要がある
- スタック上の値を使ってラベルにジャンプする命令はない
- 関数ポインタ的な動作を実現するには、ID → ラベル の分岐テーブルを生成する必要がある（実行コストが大きい）

### 1.2 解決策: テンプレート関数

C++ のテンプレートや Rust のジェネリクス（モノモーフィゼーション）に類似した手法で、**コンパイル時に**関数を具体化する。

- 関数定義にパラメータ化された「alias パラメータ」を宣言する
- 使用時に具体的な関数名・定数値で alias パラメータを束縛し、新しい関数を生成する
- 生成された関数は通常の関数と同じようにコンパイル・呼び出しされる
- Whitespace コンパイルでも追加コストなし

---

## 2. 構文設計

### 2.1 テンプレート関数定義

通常の `func:` 定義に `, alias:` 句を追加する形式。

```bnf
template_func ::=
    "func" ":" ident "(" (ident ("," ident)*)? ")" ("," alias_param)+ block

alias_param ::=
    | "alias" ":" "func" ":" ident "(" (ident ("," ident)*)? ")"   # 関数 alias パラメータ
    | "alias" ":" "constexpr" ":" ident                             # 定数 alias パラメータ
    | "alias" ":" "static" ":" ident                                # 静的変数 alias パラメータ
```

### 2.2 テンプレート関数の例

```nospace
# 関数 alias パラメータ: compare_func は引数2つの関数 #
func: sort_by(arr), alias: func: compare_func(a, b) {
  # arr を compare_func を使ってソートする #
  # compare_func(x, y) のように呼び出せる #
}

# constexpr alias パラメータ: low, high はコンパイル時定数 #
func: find_of(arr), alias: constexpr: low, alias: constexpr: high {
  # arr の中から low 以上 high 以下の要素を探す #
  # low, high は定数値として式内で使用可能 #
}

# static alias パラメータ: shared_count は外部の static 変数を参照する #
func: increment(amount), alias: static: shared_count {
  shared_count = shared_count + amount;
  return: shared_count;
}

# 複数種類の alias パラメータの混在 #
func: transform(arr, len), alias: func: mapper(x), alias: constexpr: offset {
  repeat: i(0), len, {
    arr[i] = mapper(arr[i]) + offset;
  };
}
```

### 2.3 テンプレートインスタンス化（alias 文）

既存の `alias:` 文を拡張して、テンプレート関数のインスタンス化にも対応する。

```bnf
alias_instantiation ::=
    "alias" ":" ident "(" ident ("," alias_arg)+ ")" ";"

alias_arg ::= ident | integer
```

**構文**: `alias: 新名前(テンプレート名, alias引数1, alias引数2, ...);`

- 第1引数: テンプレート関数名
- 第2引数以降: alias パラメータに対応する具体的な値（関数名・定数値）

```nospace
func: compare_string(a, b) {
  # 文字列比較の実装 #
  return: a - b;
}

# テンプレートインスタンス化 #
alias: sort_by_impl(sort_by, compare_string);
alias: find_of_impl(find_of, 10, 99);

static: count_a(0);
static: count_b(100);
alias: inc_a(increment, count_a);
alias: inc_b(increment, count_b);
```

### 2.4 インスタンス化された関数の呼び出し

```nospace
func: __main() {
  let: my_array[5]([3, 1, 4, 1, 5]);

  sort_by_impl(my_array);
  find_of_impl(my_array);

  __puti(inc_a(1));    # 1   (count_a: 0 → 1) #
  __puti(inc_a(2));    # 3   (count_a: 1 → 3) #
  __puti(inc_b(10));   # 110 (count_b: 100 → 110) #
}
```

`inc_a` と `inc_b` は異なる static 変数（`count_a`, `count_b`）を参照するため、独立した状態を持つ。同じ static 変数を渡せば状態を共有することもできる。

---

## 3. セマンティクス

### 3.1 テンプレート関数はバイナリを生成しない

テンプレート関数定義そのものはコード生成の対象にならない。`alias:` によるインスタンス化が行われたときに初めて具体的な関数が生成される。

- テンプレート関数は AST としてのみ保存される
- インスタンス化されていないテンプレートは最終バイナリに含まれない
- 未使用のインスタンスも最適化で削除可能（既存の未使用関数削除最適化と同じ）

### 3.2 alias パラメータの種類

| 種類 | 宣言構文 | インスタンス化時の引数 | テンプレート内での扱い |
|------|---------|----------------------|---------------------|
| `func:` | `alias: func: name(args...)` | 関数名（識別子） | 関数呼び出し可能。引数の数は宣言時に指定したものと一致する必要がある |
| `constexpr:` | `alias: constexpr: name` | 整数リテラルまたは constexpr 名 | 定数式として扱われる。`constexpr:` と同様に `Factor(value)` に置換される |
| `static:` | `alias: static: name` | static 変数名（識別子） | static 変数の参照（エイリアス）。テンプレート内で読み書き可能。スコープ外の static 変数にアクセスする手段を提供する |

#### `func:` alias パラメータの検証

```nospace
func: apply(x), alias: func: f(a) {
  return: f(x);
}

func: double(n) { return: n * 2; }
func: triple(n) { return: n * 3; }

alias: apply_double(apply, double);   # OK: double は引数1つの関数 #
alias: apply_triple(apply, triple);   # OK: triple は引数1つの関数 #
# alias: bad(apply, some_binary_func);  # コンパイルエラー: 引数の数が不一致 #
```

- インスタンス化時に、渡された関数の引数の数がテンプレートの宣言と一致するか検証する
- 不一致の場合はコンパイルエラー

#### `constexpr:` alias パラメータの検証

```nospace
func: offset_add(x), alias: constexpr: offset {
  return: x + offset;
}

constexpr: TEN(10);
alias: add_ten(offset_add, TEN);      # OK: constexpr #
alias: add_five(offset_add, 5);       # OK: 整数リテラル #
# alias: bad(offset_add, some_var);    # コンパイルエラー: 変数は constexpr ではない #
```

#### `static:` alias パラメータの検証

```nospace
func: accumulate(val), alias: static: acc {
  acc = acc + val;
  return: acc;
}

static: total(0);
alias: add_to_total(accumulate, total);    # OK: total は static 変数 #
# alias: bad(accumulate, local_var);        # コンパイルエラー: static 変数ではない #
# alias: bad2(accumulate, 42);              # コンパイルエラー: リテラルは static 変数ではない #
```

- インスタンス化時に、渡された識別子が static 変数であることを検証する
- let 変数やリテラルを渡した場合はコンパイルエラー
- テンプレート内では `acc` を通常の変数のように読み書きできる（実体は渡された static 変数）

### 3.3 スコープの独立性

各インスタンスはそれぞれ独立した関数として生成される。テンプレート内の `static:` 変数もインスタンスごとに独立する。

```nospace
func: counter(), alias: constexpr: step {
  static: count(0);
  count = count + step;
  return: count;
}

alias: inc1(counter, 1);
alias: inc5(counter, 5);

func: __main() {
  __puti(inc1());   # 1  (inc1 の count: 0 → 1) #
  __puti(inc1());   # 2  (inc1 の count: 1 → 2) #
  __puti(inc5());   # 5  (inc5 の count: 0 → 5) ... 独立 #
  __puti(inc5());   # 10 (inc5 の count: 5 → 10) #
  __puti(inc1());   # 3  (inc1 の count: 2 → 3) ... 独立 #
}
```

**実装**: インスタンス化時に AST をクローンし、独立した関数として意味解析・コード生成を行う。

### 3.4 名前解決とスコープ

テンプレート関数内の名前解決は以下の順序で行われる:

1. alias パラメータ（`func:` は関数名、`constexpr:` は定数値、`static:` は static 変数の参照として解決）
2. テンプレート関数自身の引数・ローカル変数
3. テンプレート関数が**定義されたスコープ**の変数・関数（通常の関数と同じ）

これは**定義元スコープ**での解決であり、ブロックエイリアスの「呼び出し元スコープ」展開とは異なる。テンプレート関数はあくまで「関数」であり、関数スコープの規則に従う。

### 3.5 テンプレートのインスタンス化の処理フロー

```
1. tree_parser:
   - テンプレート関数定義を AST ノードとしてパースし保存
   - alias インスタンス化文をパース

2. semantic_analyzer (Pass 0):
   - テンプレート関数定義を収集（通常の関数とは別に管理）
   - alias インスタンス化文を処理:
     a. 対応するテンプレート関数を検索
     b. alias 引数の数・種類を検証
     c. テンプレートの AST をクローン
     d. alias パラメータを具体的な値に置換:
        - func: → 関数名の alias マッピングを設定
        - constexpr: → constexpr テーブルにエントリ追加        - static: → static 変数への識別子エイリアスを設定（alias_map に登録）     e. クローンした AST を新しい関数定義として登録

3. semantic_analyzer (Pass 1a, 1b, 2):
   - インスタンス化された関数は通常の関数として処理される

4. compiler_ws / interpreter:
   - 変更不要（通常の関数として生成済み）
```

### 3.6 テンプレート関数の直接呼び出しの禁止

テンプレート関数をインスタンス化せずに直接呼び出すとコンパイルエラーとなる。

```nospace
func: tmpl(x), alias: constexpr: n {
  return: x + n;
}

func: __main() {
  # tmpl(5);   # コンパイルエラー: テンプレート関数は直接呼び出せない #
}
```

### 3.7 ホイスティング

- テンプレート関数定義はスコープ内でホイスティングされる（通常の `func:` と同じ）
- テンプレートインスタンス化（`alias:` 文）もホイスティングされる（既存の alias と同じ）

```nospace
func: __main() {
  __puti(my_add(3, 4));  # OK: ホイスティングされる #
}

alias: my_add(tmpl_add, 10);

func: tmpl_add(a, b), alias: constexpr: bias {
  return: a + b + bias;
}
```

---

## 4. 既存機能との関係

### 4.1 alias（識別子エイリアス）との関係

既存の `alias: name(target)` は単純な名前置換であり、テンプレートインスタンス化とは別の機能。

**構文の共存**:
- `alias: name(identifier)` → 引数が1つの場合
  - `identifier` がテンプレート関数名であり、そのテンプレートが alias パラメータを持つ場合 → **コンパイルエラー**（alias 引数の数が不足）
  - `identifier` がテンプレート関数名でない場合 → **既存の識別子エイリアス**として処理
- `alias: name(identifier, arg1, ...)` → 引数が2つ以上の場合
  - `identifier` がテンプレート関数名 → **テンプレートインスタンス化**
  - `identifier` がテンプレート関数名でない場合 → **コンパイルエラー**

**判別ロジック**:
```
parse_alias_statement(name, args):
  if len(args) == 1:
    target = args[0]
    if target ∈ template_functions:
      template = template_functions[target]
      if len(template.alias_params) == 0:
        # 0 個の alias パラメータ → テンプレート関数ではなく通常関数
        # 識別子エイリアスとして処理
        → AliasIdentifier(name, target)
      else:
        → コンパイルエラー: "template '{target}' requires {n} alias arguments, but 0 were provided"
    else:
      → AliasIdentifier(name, target)
  else:
    template_name = args[0]
    alias_args = args[1:]
    if template_name ∉ template_functions:
      → コンパイルエラー: "'{template_name}' is not a template function"
    template = template_functions[template_name]
    if len(alias_args) != len(template.alias_params):
      → コンパイルエラー: "alias argument count mismatch"
    → TemplateInstantiation(name, template_name, alias_args)
```

### 4.2 alias（ブロックエイリアス）との関係

ブロックエイリアスはマクロ的なインライン展開であり、テンプレート関数はあくまで関数生成である。使い分け:

| 特徴 | ブロックエイリアス | テンプレート関数 |
|------|-------------------|----------------|
| 展開タイミング | 呼び出し時にインライン展開 | インスタンス化時に関数生成 |
| スコープ | 呼び出し元スコープ | 定義元スコープ（関数スコープ） |
| パラメータ | なし | alias パラメータで関数・定数を注入 |
| static 変数 | 呼び出し元依存 | インスタンスごとに独立（static: alias で外部から注入も可） |
| 再帰 | 不可（巡回検知でエラー） | 通常の関数として再帰可能 |
| 用途 | 短いコード片の繰り返し | 汎用アルゴリズムのパラメータ化 |

### 4.3 constexpr / static との関係

- テンプレートの `alias: constexpr: name` パラメータは、インスタンス化時に `constexpr:` エントリとして登録される
- 既存の constexpr 評価器をそのまま利用可能
- テンプレートの `alias: static: name` パラメータは、インスタンス化時に識別子エイリアスとして登録される
- 既存の識別子 alias 解決機構を利用可能
- `constexpr:` と `static:` の違い: `constexpr:` はコンパイル時定数に置換される（変数実体なし）、`static:` は実在する変数への参照（読み書き可能）

---

## 5. 実装計画

### 5.1 前提条件

以下が実装済みであること:
- [x] alias（識別子エイリアス）— Step 3 in [unimplemented-variable-features.md](unimplemented-variable-features.md)
- [x] constexpr（式形式）— Step 2 in [unimplemented-variable-features.md](unimplemented-variable-features.md)

### 5.2 ステップ一覧

| Step | 内容 | 依存 | 状態 |
|------|------|------|------|
| 1 | AST 定義の拡張（テンプレート関数ノード） | なし | ❌ 未実装 |
| 2 | tree_parser: テンプレート関数定義のパース | Step 1 | ❌ 未実装 |
| 3 | tree_parser: テンプレートインスタンス化文のパース | Step 1 | ❌ 未実装 |
| 4 | semantic_analyzer: テンプレート収集・インスタンス化 | Step 2, 3 | ❌ 未実装 |
| 5 | テストケース追加 | Step 4 | ❌ 未実装 |
| 6 | spec.md / grammar.bnf への反映 | Step 4 | ❌ 未実装 |

**依存関係**:
```
Step 1 → Step 2 → Step 4
Step 1 → Step 3 → Step 4
Step 4 → Step 5
Step 4 → Step 6
```

### 5.3 Step 1: AST 定義の拡張

**目的**: テンプレート関数定義とインスタンス化を表現する AST ノードを追加する。

#### 新規 AST ノード

**tree_parser/statement/mod.rs**:

```rust
/// テンプレート関数の alias パラメータの種類
#[derive(Debug, Clone, PartialEq)]
pub enum AliasParamKind {
    /// alias: func: name(arg1, arg2, ...) — 関数パラメータ（引数名リスト付き）
    Func(Vec<String>),
    /// alias: constexpr: name — コンパイル時定数パラメータ
    Constexpr,
    /// alias: static: name — static 変数参照パラメータ（外部 static 変数への読み書きアクセス）
    Static,
}

/// テンプレート関数の alias パラメータ定義
#[derive(Debug, Clone, PartialEq)]
pub struct AliasParam {
    pub name: String,
    pub kind: AliasParamKind,
}

// Statement enum に追加:
// Statement::TemplateFunctionDefinition {
//     name: String,
//     args: Vec<String>,
//     alias_params: Vec<AliasParam>,
//     body: Vec<LocatedStatement>,
// }

// Statement::AliasInstantiation {
//     name: String,               // 新しい関数名
//     template_name: String,      // テンプレート関数名
//     alias_args: Vec<AliasArg>,  // alias 引数
// }
```

**AliasArg**:

```rust
/// テンプレートインスタンス化時の alias 引数
#[derive(Debug, Clone, PartialEq)]
pub enum AliasArg {
    /// 関数名や変数名（識別子）
    Identifier(String),
    /// 整数リテラルや数値式
    Value(i64),
}
```

#### 既存 AST との統合方針

テンプレート関数定義は既存の `Statement::FunctionDefinition` とは別の variant として追加する理由:

- テンプレート関数は通常の関数とは処理パスが異なる（直接コード生成しない）
- alias パラメータという追加情報を持つ
- 意味解析での処理が大きく異なる

ただし、インスタンス化後は `Statement::FunctionDefinition` として扱われる。

### 5.4 Step 2: テンプレート関数定義のパース

**目的**: `func: name(args...), alias: kind: param...` 形式のパースを実装する。

**変更対象**: `src/tree_parser/statement/mod.rs`

#### パース規則

```
"func" ":" ident "(" param_list ")" ("," alias_param)+ block → TemplateFunctionDefinition
```

通常の `func:` パースのフローを拡張:

```
parse_func:
  keyword "func"
  ":"
  name = ident
  "(" params ")"
  if next_token == ",":       # ← ここで分岐
    alias_params = parse_alias_params()
    block = parse_block()
    → TemplateFunctionDefinition(name, params, alias_params, block)
  else:
    block = parse_block()
    → FunctionDefinition(name, params, block)
```

`alias_param` のパース:

```
parse_alias_param:
  "alias" ":"
  kind = match:
    "func" ":"    → parse_func_alias_param()   # name(args...)
    "constexpr" ":" → Constexpr, ident
    "static" ":"  → Static, ident
  return AliasParam(kind, name)
```

### 5.5 Step 3: テンプレートインスタンス化文のパース

**目的**: `alias: new_name(template, arg1, arg2, ...);` 形式との区別・パースを実装する。

**変更対象**: `src/tree_parser/statement/mod.rs`

#### 既存の alias パースとの統合

既存: `alias: name(target);` → `AliasIdentifier(name, target)`

拡張: `alias: name(target, arg1, arg2, ...);` → 引数が2つ以上の場合

```
parse_alias:
  "alias" ":"
  if next_token == "{":
    → 既存のブロックエイリアスパース
  else:
    name = ident
    "("
    first_arg = parse_alias_arg()
    if next_token == ",":
      args = [first_arg]
      while next_token == ",":
        ","
        args.push(parse_alias_arg())
      ")" ";"
      → AliasInstantiation(name, first_arg_as_template, args[1:])
    else:
      ")" ";"
      → AliasIdentifier(name, first_arg_as_ident)
```

**注**: tree_parser の段階ではテンプレート関数かどうかの判別は行わず、 引数の数で AST ノードの種類を区別する。semantic_analyzer でテンプレート関数の存在を検証する。

ただし、引数が1つの場合でテンプレート関数への呼び出しを意図しているケースもあり得るため、以下の方針とする:

- **tree_parser**: 引数が1つ → `AliasIdentifier`、引数が2つ以上 → `AliasInstantiation` として一旦パース
- **semantic_analyzer**: `AliasIdentifier` のターゲットがテンプレート関数であれば、alias パラメータの数を検証してエラーを報告

### 5.6 Step 4: semantic_analyzer の変更

**目的**: テンプレート関数の収集・インスタンス化・関数生成を実装する。

**変更対象**: `src/semantic_analyzer/mod.rs`, `src/semantic_analyzer/scope.rs`

#### 処理フロー

```
Pass 0 (拡張):
  1. テンプレート関数定義の収集
     - TemplateFunctionDefinition を template_table に登録
     - AST（body）をそのまま保存
     - 通常の関数としては登録しない

  2. テンプレートインスタンス化の処理
     - AliasInstantiation を検出
     - template_table からテンプレートを取得
     - alias 引数の数・種類を検証:
       - func: → 引数が関数名であること、引数の数が宣言と一致すること
       - constexpr: → 引数が定数式であること
       - static: → 引数が static 変数名であること
     - テンプレートの AST をクローン
     - alias パラメータのマッピングを生成:
       - func: param → alias: param(concrete_func) as alias map entry
       - constexpr: / static: param → constexpr: param(value) as constexpr table entry
     - クローンした AST を FunctionDefinition として登録:
       - 関数名: インスタンス化時に指定した新名前
       - 引数: テンプレートの引数リスト（そのまま）
       - body: クローンした body（alias マッピング付きのスコープで処理）

Pass 1a (既存):
  - インスタンス化された関数は通常の関数として登録・処理される

Pass 1b (既存):
  - 変更不要

Pass 2 (既存):
  - インスタンス化された関数内の alias 解決は既存の alias 機構を利用
  - constexpr 解決も既存の constexpr 機構を利用
```

#### scope.rs の変更

```rust
// ScopeInfo に追加
pub struct ScopeInfo {
    // ... 既存フィールド ...
    /// テンプレート関数テーブル
    pub template_table: BTreeMap<String, TemplateEntry>,
}

pub struct TemplateEntry {
    pub name: String,
    pub args: Vec<String>,
    pub alias_params: Vec<AliasParam>,
    pub body: Vec<LocatedStatement>,
}
```

#### インスタンス化された関数のスコープ構造

```
生成される関数のスコープ:
  - alias_map: { alias_func_param → concrete_func, alias_static_param → concrete_static_var }
  - constexpr_table: { alias_constexpr_param → concrete_value }
  - 上記以外は通常の関数スコープと同じ

注: static: alias パラメータは alias_map に識別子エイリアスとして登録される。
テンプレート内で `name` を使用すると、渡された static 変数名に解決される。
static 変数は関数スコープの境界を超えてアクセス可能であるため、
通常の alias 解決ロジック（既存の識別子エイリアス）でそのまま動作する。
```

### 5.7 Step 5: テストケース追加

#### 成功テスト

| テスト名 | 内容 |
|---------|------|
| `template_func_basic_001` | constexpr alias パラメータの基本動作 |
| `template_func_func_alias_001` | func alias パラメータの基本動作 |
| `template_func_multi_alias_001` | 複数の alias パラメータ |
| `template_func_mixed_alias_001` | func + constexpr 混在 |
| `template_func_static_alias_001` | static alias パラメータで外部 static 変数を参照 |
| `template_func_static_shared_001` | 複数インスタンスで同一 static 変数を共有 |
| `template_func_static_independent_001` | 異なる static 変数を渡して独立動作 |
| `template_func_hoisting_001` | テンプレート定義・インスタンス化のホイスティング |
| `template_func_nested_001` | テンプレート内でのネスト関数定義 |
| `template_func_recursive_001` | インスタンス化された関数の再帰呼び出し |

#### エラーテスト

| テスト名 | 内容 |
|---------|------|
| `template_func_direct_call_001` | テンプレート関数の直接呼び出しエラー |
| `template_func_arg_mismatch_001` | alias 引数の数が不一致 |
| `template_func_func_arity_001` | func alias の引数数不一致 |
| `template_func_non_const_001` | constexpr alias に変数を渡すエラー |
| `template_func_static_non_static_001` | static alias に let 変数を渡すエラー |
| `template_func_static_literal_001` | static alias にリテラルを渡すエラー |
| `template_func_not_template_001` | 通常関数に対するインスタンス化エラー |

### 5.8 Step 6: ドキュメント反映

- `docs/spec.md` に「テンプレート関数」セクションを追加
- `docs/grammar.bnf` に BNF 規則を追加
- `docs/tutorial.md` にテンプレート関数の使い方を追加

---

## 6. 設計上の未決定事項

### 6.1 `static:` alias パラメータの実装詳細

**決定済み**: `static:` alias パラメータは外部の static 変数への参照として機能する。`constexpr:` とは明確に異なるセマンティクスを持つ。

**ライフタイムの安全性**: static 変数に限定しているため、参照先がスコープ外で破棄されるライフタイム問題は発生しない。static 変数はプログラム全体の寿命を持つため、いつテンプレート関数が呼び出されても安全にアクセスできる。

**実装方針**: 意味解析器の既存の識別子エイリアス解決機構を利用する。`static: param` → `alias: param(concrete_static_var)` としてエイリアスマップに登録すれば、既存の名前解決で正しく動作する。ただし、static 変数であることの検証（let 変数やリテラルの拒否）は追加で必要。

**検討点**:
- グローバル変数（グローバルスコープの let は static 同等）も `static:` alias で渡せるべきか？
  - グローバル let も static と同じライフタイムを持つため、許可するのが自然

### 6.2 テンプレートからテンプレートの呼び出し

テンプレート関数内で別のテンプレートをインスタンス化することを許可するか？

```nospace
func: inner(x), alias: constexpr: n { return: x + n; }
func: outer(x), alias: constexpr: m {
  alias: my_inner(inner, m);     # テンプレート内でのインスタンス化 #
  return: my_inner(x);
}
```

→ 初回は禁止とする。テンプレート内では通常の alias 文のみ許可。

### 6.3 テンプレートの型チェック

現在 nospace は int / void の2型のみ。テンプレート関数の alias パラメータに対する型チェックは以下に限定:

- `func:` パラメータ: 引数の数のみチェック（型チェックなし）
- `constexpr:` パラメータ: コンパイル時定数であることのチェック
- `static:` パラメータ: static 変数であることのチェック（読み書き可能な変数参照）

将来的に型システムが拡張された場合、テンプレートの型制約も拡張が必要。

### 6.4 テンプレートの再帰的インスタンス化

```nospace
alias: fib_0(tmpl_fib, 0);
alias: fib_1(tmpl_fib, 1);
alias: fib_n(tmpl_fib, N);  # N が前のインスタンスを参照 → 無限展開の可能性 #
```

テンプレートのインスタンス化は alias 引数が具体的な値（リテラルまたは constexpr）であるため、再帰的なインスタンス化は発生しない。ただし、将来的に式をインスタンス化引数として許可した場合は検討が必要。

### 6.5 名前マングリング

Whitespace コンパイル時のラベル生成で、インスタンス化された関数に一意なラベルが必要。

- 既存の関数ラベル生成ロジックをそのまま利用可能
- インスタンス化された関数は `alias:` 文で指定された名前で登録されるため、通常の関数と同じラベル付けルールが適用される
- 名前の衝突は既存の重複定義チェックで検出される

### 6.6 ネストされたスコープでのテンプレート定義

テンプレート関数を関数内で定義することを許可するか？

```nospace
func: __main() {
  func: tmpl(x), alias: constexpr: n { return: x + n; }
  alias: add5(tmpl, 5);
  __puti(add5(10));  # 15 #
}
```

→ 通常の関数定義がネスト可能であるため、テンプレート関数もネスト可能とする。スコープ規則は通常のネスト関数と同じ。

---

## 関連ドキュメント

- [unimplemented-variable-features.md](unimplemented-variable-features.md) - alias / constexpr の設計（テンプレート機能の前提）
- [docs/spec.md](../../docs/spec.md) - 言語仕様
- [docs/grammar.bnf](../../docs/grammar.bnf) - 文法定義
- [ai-docs/architecture/overview.md](../architecture/overview.md) - アーキテクチャ概要

---

## 更新履歴

- 2026-02-28: `static:` alias パラメータを「constexpr 同等」から「static 変数への参照」に変更
- 2026-02-28: 初版作成。テンプレート関数の設計・構文・セマンティクス・実装計画を記載
