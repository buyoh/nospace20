# import 文の設計

## 概要

`import:` 文は、名前空間内で他の名前空間に定義された識別子を直接利用可能にする機構。C++ の `using namespace` に相当する。

## 言語仕様

### 基本構文

```bnf
import_stmt ::= "import" ":" import_modifier* ident ";"
import_modifier ::= "weak" ":" | "export" ":"
```

### 基本動作

`import: N1;` は、名前空間 `N1` に定義されたすべての識別子を、現在の名前空間内で直接参照可能にする。

```nospace
namespace: N1 {
  let: n1(10);
  let: n2(20);
}

namespace: M2 {
  import: N1;
  __clog(n1);  # 10 #
  # let: n2(99);  エラー: N1$n2 と衝突 #
}

# __clog(M2$n1);  エラー: M2$n1 は定義されていない #
```

**ポイント:**

- `import:` は「名前解決の短縮」であり、識別子の再定義ではない
- import により参照可能になった識別子は、空間外部からは見えない（`M2$n1` のような修飾アクセスは不可）
- 現在の名前空間で既に定義されている識別子と衝突する場合はコンパイルエラー

### 修飾子

#### `weak:` 修飾子

同名識別子が現在の名前空間に既に存在する場合、エラーにせず既存の識別子を優先する。

```nospace
namespace: N1 {
  let: n1(10);
  let: n2(20);
}

namespace: M2 {
  import: weak: N1;
  let: n1(99);
  __clog(n1);  # 99 (M2$n1 を優先) #
  __clog(n2);  # 20 (N1$n2 を参照) #
}
```

#### `export:` 修飾子

import した識別子を、外部から修飾名でアクセス可能にする。

```nospace
namespace: N1 {
  let: n1(10);
  let: n2(20);
}

namespace: M2 {
  import: export: N1;
  __clog(n2);  # 20 #
}

__clog(M2$n2);  # 20 (export により M2$n2 として参照可能) #
```

#### 修飾子の組み合わせ

`weak:` と `export:` は組み合わせ可能。順序は任意。

```nospace
namespace: N1 {
  let: n1(10);
  let: n2(20);
}

namespace: M2 {
  import: weak: export: N1;
  let: n1(99);
  __clog(n1);  # 99 (M2$n1 を優先) #
  __clog(n2);  # 20 (N1$n2 を参照可能) #
  __clog(N1$n1);  # 10 (N1$n1 を参照) #
}

__clog(M2$n1);  # 99 (M2$n1 を参照) #
__clog(M2$n2);  # 20 (export により M2$n2 として参照可能) #
```

### import 対象の識別子種別

名前空間に定義可能なすべての種別が import 対象となる:

| 種別 | import 可能 | 備考 |
|------|------------|------|
| `let:` / `static:` / `final:` 変数 | 可 | |
| `func:` 関数 | 可 | |
| `constexpr:` 定数 | 可 | |
| `alias:` エイリアス | 可 | |
| ネストした名前空間の識別子 | 不可 | `N1` 直下の識別子のみ。`N1` 内のネスト名前空間の中身は対象外 |

### 制約・エラーケース

| ケース | 動作 |
|--------|------|
| import 対象の名前空間が未定義 | コンパイルエラー |
| import した識別子と同名の識別子が既に定義済み（`weak:` なし） | コンパイルエラー |
| import した識別子と同名の識別子が既に定義済み（`weak:` あり） | 既存を優先、エラーにしない |
| グローバルスコープでの `import:` | 許可（名前空間外でも使用可能） |
| import の import（A が B を import し、C が A を import） | A の import 結果は C には伝播しない。A に直接定義された識別子のみ |
| `export:` で公開された識別子の更に外側からのアクセス | 修飾名でアクセス可能 |
| 同一名前空間を複数回 import | コンパイルエラー（重複 import） |
| 自身の名前空間を import | コンパイルエラー |

### グローバルスコープでの import

名前空間ブロック外（グローバルスコープ）でも import は使用可能。

```nospace
namespace: Utils {
  func: helper() { return: 42; }
}

import: Utils;
func: __main() {
  __clog(helper());  # 42 #
}
```

この場合:
- `weak:` / `export:` は不要（グローバルでは修飾無しで常にアクセス可能）
- `export:` を付けても意味的には変わらないがエラーにはしない

## 実装設計

### 全体方針

`import:` は意味解析（semantic_analyzer）の段階で処理される、コンパイル時の名前解決機構である。ランタイムへの影響はない。

既存の名前空間マングリング機構の上に、import 情報を追加するアプローチをとる。

### Step 1: Token Parser (`src/token_parser/`)

#### 変更内容

`Keyword` enum に `Import` を追加する。

```rust
pub enum Keyword {
    // ... 既存 ...
    Import,
}
```

- `as_keyword_token`: `"import" => Some(Token::Keyword(Keyword::Import))`
- `as_str`: `Keyword::Import => "import"`

`weak` と `export` はキーワードとして追加する。

```rust
pub enum Keyword {
    // ... 既存 ...
    Weak,
    Export,
}
```

- `as_keyword_token`: `"weak" => Some(Token::Keyword(Keyword::Weak))`
- `as_keyword_token`: `"export" => Some(Token::Keyword(Keyword::Export))`
- `as_str`: `Keyword::Weak => "weak"`
- `as_str`: `Keyword::Export => "export"`

`weak` と `export` は既存の識別子名として使用されている可能性がある。ただし nospace では予約語の扱いが「キーワード＋コロン」方式のため、`weak:` `export:` として使われない限り識別子として解釈される。トークナイザレベルでは Keyword として登録し、パーサが文脈で判別する既存方針と一貫する。

→ **検討**: `weak` / `export` を Keyword にすると、これらの名前の変数や関数が使えなくなるリスクがある。ただし、nospace の既存キーワード（`let`, `func`, `if` 等）もすべてこの方式で、「`keyword:` コロン付きのとき」のみキーワード扱いとなるため、`weak` / `export` を識別子として使い続けることは可能。

**確認事項**: トークナイザが `weak` を `Token::Keyword(Keyword::Weak)` として返す場合、パーサは `weak` が識別子として使われるコンテキスト（変数名 `weak` 等）で `Token::Keyword(Keyword::Weak)` を識別子に読み替える必要がある。既存の実装（`let`, `if` 等）ではキーワードはコロン付きでしかパースされないため、コロン無しの `Token::Keyword(...)` は識別子として扱われる仕組みが存在するか確認する。

→ 既存の tree_parser では、`expr_val` の識別子パースで `Token::Identifier` のみマッチしており、`Token::Keyword` は式中で識別子として扱われない。そのため、`weak` / `export` を Keyword に追加すると `let: weak(10);` のような変数宣言が壊れるリスクがある。

**代替案**: `weak` / `export` を独立 Keyword にせず、import 文パース時にのみ識別子として検出する方式を検討する:

```
import: の後:
  ident("weak") + ":" → weak フラグ ON
  ident("export") + ":" → export フラグ ON
  ident → 対象の名前空間名
```

この方式ならトークナイザの変更は `Import` キーワードの追加のみで済み、`weak` / `export` は通常の識別子として扱われ続ける。

**推奨**: `weak` / `export` は Keyword に追加せず、パーサ側で文脈依存的に識別子を検出する方式を採用する。これにより既存コードへの影響を最小化できる。ただし、一方で `static:` `final:` `constexpr:` など既存の修飾子は Keyword として定義されている点との一貫性に注意する。

→ **最終決定**: `Import`, `Weak`, `Export` とも Keyword enum に追加する。ただし、`Weak` / `Export` が識別子として使われるケースのために、tree_parser 側でキーワードの識別子フォールバック処理を確認し、必要に応じて修正する。

### Step 2: Tree Parser (`src/tree_parser/`)

#### 変更内容

1. **`Statement` enum に `ImportDeclaration` を追加**

```rust
pub enum Statement {
    // ... 既存 ...
    /// import 宣言: `import: [weak:] [export:] Namespace;`
    ImportDeclaration {
        namespace_name: String,
        is_weak: bool,
        is_export: bool,
    },
}
```

2. **パース処理**

`parse_to_statements` のキーワードマッチに `Keyword::Import` を追加:

```
Keyword::Import の後:
  1. Colon を消費
  2. 修飾子をチェック:
     - Keyword::Weak + Colon → is_weak = true
     - Keyword::Export + Colon → is_export = true
     (順序は任意、各修飾子は最大1回)
  3. 識別子を読み取り → namespace_name
  4. Semicolon を消費
```

**修飾子のコロン付き解析**: `weak:` / `export:` は既存の「キーワード＋コロン」パターンに従うため、パースは自然に行える。

3. **キーワードの識別子フォールバック**

`weak` / `export` がキーワードとして認識されることで、変数名・関数名として使えなくなる場合、tree_parser の識別子パース部分（`expr_val` の `Token::Identifier` マッチ）に `Token::Keyword(Keyword::Weak)` / `Token::Keyword(Keyword::Export)` を識別子として受け入れるフォールバックを追加する必要がある。

→ ただし、既存のキーワード（`let`, `func` 等）はすでに変数名として使えないため、`weak` / `export` も同様にキーワード予約とするのが一貫性がある。`weak` / `export` を変数名として使うユースケースは稀と想定。

### Step 3: Semantic Analyzer (`src/semantic_analyzer/`)

これが最も大きな変更。import の解決はマングリング機構と連携して行う。

#### 3.1 Import 情報の収集（新しいパス0の前段階）

既存のパス構成:

- パス0: constexpr / alias 収集
- パス1a: 関数宣言スキャン
- パス1b: 変数宣言スキャン
- パス2: 文の変換

import の処理タイミング:

`import:` はパス0（constexpr/alias）と同時期、もしくはパス0の前に処理する必要がある。理由: import された識別子が constexpr や alias のターゲットとして参照される可能性があるため。

ただし import の解決には対象名前空間内の識別子一覧が必要であり、これはパス0〜1b で収集される。

**方針**: 2段階処理

1. **パス0前半（import 収集）**: import 文を再帰的にスキャンし、import 情報を `ImportInfo` として収集する
2. **パス0後半（識別子収集）**: constexpr / alias / 関数 / 変数のスキャン時に import 情報を考慮して衝突チェックと名前登録を行う

→ **改善案**: import は名前解決の「別名（alias 的）」として実装できる。import された各識別子に対して、内部的に alias エントリを生成することで、既存の alias 解決パイプラインを再利用できる。

#### 3.2 実装方式: alias 展開方式

import 文を処理する際、対象名前空間の識別子を列挙し、それぞれに対して以下を行う:

- **変数**: `ScopeResolver` の名前解決で import 元を検索するための情報を追加
- **関数**: 同上
- **constexpr**: alias テーブルに追加（`name → NS$name`）
- **alias**: alias テーブルに追加（`name → NS$name`、チェーン解決）

しかし、変数と関数はスロットインデックスベースで管理されており、alias テーブルとは管理方式が異なる。

#### 3.3 実装方式: ScopeResolver の名前解決拡張

**推奨**: import 情報を `ScopeResolver` に保持し、名前解決時に import テーブルを参照する方式。

```rust
/// import 情報
struct ImportEntry {
    /// import 元の名前空間プレフィックス（例: "N1$"）
    source_prefix: String,
    /// import 先の名前空間プレフィックス（例: "M2$"）
    target_prefix: String,
    /// weak フラグ
    is_weak: bool,
    /// export フラグ
    is_export: bool,
}
```

名前解決の拡張:

1. 既存ロジック: `ns_candidate_names` で候補を生成し探索
2. **追加**: 見つからない場合、import テーブルを参照し、`source_prefix + name` で再探索

#### 3.4 処理フロー詳細

**フェーズ A: import 文の収集**

全ステートメントを事前スキャンし、名前空間＋import の対応を収集する。

```rust
/// 名前空間ごとの import 情報を収集する
fn collect_imports(
    statements: &[LocatedStatement],
    ns_prefix: &str,
) -> Result<Vec<ImportEntry>, Vec<CodeParseError>>
```

この段階では対象名前空間が定義済みかどうかのチェックも行う。

**フェーズ B: 衝突チェック**

import された識別子が、現在の名前空間に既に定義されている識別子と衝突しないか検証する。

- `is_weak == false` の場合: 衝突があればコンパイルエラー
- `is_weak == true` の場合: 衝突があっても既存を優先

衝突チェックには、対象名前空間の直下の識別子一覧が必要。名前空間 `N1` の直下の識別子は、prefix `N1$` で始まり、かつそれ以上の `$` を含まない（ネストしていない）ものの名前部分（prefix を除いた部分）。

**フェーズ C: 名前解決テーブルの構築**

1. import 対象の各識別子について:
   - **export なし**: `ScopeResolver` の内部 import テーブルにのみ登録（外部参照不可）
   - **export あり**: 対象プレフィックスで alias を生成する（`M2$name → N1$name`）

2. import テーブルは constexpr / alias / 変数 / 関数の全種別に対応する。

**フェーズ D: 名前解決時の参照**

`ScopeResolver.resolve_variable` / `resolve_function` 等の名前解決で:

1. まず通常の解決を試みる
2. 見つからない場合、現在の名前空間に対する import テーブルをチェック
3. import テーブルに名前があれば、`source_prefix + name` で再度解決

#### 3.5 export の実装

`export:` フラグがある場合、import した各識別子を「修飾名でもアクセス可能」にする。

実装方式:
- 変数: `target_prefix + name` で alias エントリを追加するか、変数マップに別名を登録
- 関数: `identifier_map` に `target_prefix + name` の別名を追加
- constexpr: `constexpr_table` に `target_prefix + name` を追加
- alias: `alias_map` に `target_prefix + name → source_prefix + name` を追加

export の場合、実質的には `alias: name(Source$name);` と同等の操作を各識別子に対して行う。

#### 3.6 ScopeResolver へのフィールド追加

```rust
pub(super) struct ScopeResolver<'a> {
    pub scope_stack: Vec<ScopeInfo<'a>>,
    pub namespace_prefix: Vec<String>,
    /// import テーブル: 名前空間プレフィックス → ImportEntry のリスト
    pub import_table: Vec<ImportEntry>,
}
```

#### 3.7 名前空間直下の識別子の列挙

import 処理のために、指定のプレフィックスで始まる識別子のうち「直下」のもの（ネストした名前空間の中身を含まない）を列挙する関数が必要。

```rust
/// 名前空間 `ns_prefix` の直下に定義された識別子名（プレフィックスを除いた部分）を返す
fn enumerate_direct_members(
    ns_prefix: &str,  // 例: "N1$"
    scope: &ScopeBuilder,
    constexpr_table: &BTreeMap<String, i64>,
    alias_map: &BTreeMap<String, String>,
) -> Vec<String>
```

判定ロジック: 名前が `ns_prefix` で始まり、`ns_prefix` を除いた残りに `$` を含まないもの。

### Step 4: Compiler WS / Interpreter / Optimizer

変更なし。import は意味解析でマングリング済みの名前に解決されるため、後段への影響はない。

### Step 5: Grammar / Syntax

#### `syntaxes/grammar.bnf` の更新

```bnf
import_stmt ::= "import" ":" import_modifier* ident ";"
import_modifier ::= "weak" ":" | "export" ":"
```

#### `syntaxes/nospace.tmLanguage.json` の更新

`import`, `weak`, `export` キーワードのハイライト定義を追加。

### Step 6: `docs/spec.md` の更新

namespace セクションに import の仕様を追記。

## 影響範囲まとめ

| モジュール | 変更 | 規模 |
|------------|------|------|
| `src/token_parser/` | `Import`, `Weak`, `Export` キーワード追加 | 小 |
| `src/tree_parser/` | `ImportDeclaration` パースの追加 | 小 |
| `src/semantic_analyzer/` | import 収集・衝突チェック・名前解決拡張・export 処理 | 大 |
| `src/compiler_ws/` | なし | - |
| `src/interpreter/` | なし | - |
| `src/optimizer/` | なし | - |
| `syntaxes/` | BNF・tmLanguage 更新 | 小 |
| `docs/spec.md` | 仕様追記 | 小 |
| `resources/tests/` | テストケース追加 | 中 |

## 実装順序

1. Step 1: Token Parser — `Import`, `Weak`, `Export` キーワード追加
2. Step 2: Tree Parser — `ImportDeclaration` のパース
3. Step 3: Semantic Analyzer — import 解決の実装（最大の作業量）
   - 3a: import 文の収集
   - 3b: 衝突チェック
   - 3c: 名前解決テーブル構築
   - 3d: export 処理
4. Step 5: Grammar / Syntax 更新
5. Step 6: spec.md 更新
6. テストケース追加・動作確認

## テスト計画

### Unit Tests

- `test_import_keyword`: `import:` キーワードのトークン化
- `test_weak_keyword`: `weak:` キーワードのトークン化
- `test_export_keyword`: `export:` キーワードのトークン化
- `test_parse_import_basic`: `import: N1;` のパース
- `test_parse_import_weak`: `import: weak: N1;` のパース
- `test_parse_import_export`: `import: export: N1;` のパース
- `test_parse_import_weak_export`: `import: weak: export: N1;` のパース

### Large Tests（成功ケース）

| ファイル名 | 内容 |
|------------|------|
| `import-basic.ns` | 基本的な import（変数・関数の参照） |
| `import-weak.ns` | weak 修飾子（衝突時の優先順位確認） |
| `import-export.ns` | export 修飾子（外部からの修飾名アクセス） |
| `import-weak-export.ns` | weak + export の組み合わせ |
| `import-constexpr.ns` | constexpr の import |
| `import-alias.ns` | alias の import |
| `import-function.ns` | 関数の import |
| `import-global.ns` | グローバルスコープでの import |

### Large Tests（失敗ケース）

| ファイル名 | 内容 |
|------------|------|
| `import-collision.ns` | 衝突エラー（weak なし） |
| `import-undefined-ns.ns` | 未定義名前空間の import |
| `import-no-external-access.ns` | export なしで外部からアクセス（エラー） |
| `import-duplicate.ns` | 同一名前空間の重複 import |
| `import-self.ns` | 自身の名前空間の import |

## 設計上の判断ポイント

### Q1: import は宣言順序に依存するか？

**A**: はい。import 文は、対象名前空間が定義された後に記述する必要がある。名前空間はホイスティングされないため、import 時点で対象名前空間の識別子が確定している必要がある。

ただし、名前空間の「内部」のホイスティングは有効（関数は名前空間内でホイスティングされる）。import の解としては全パスの結果を総合するため、パス1a・1b で登録されたすべての識別子が import 対象となる。

→ **再検討**: 実際には、意味解析のパス0〜1b で名前空間内のすべての識別子が事前にスキャンされるため、import 文の位置に関わらず対象名前空間のすべての識別子が利用可能。ただし、import 文自体がどのパスで処理されるかによる。

**最終方針**: import 文の処理はパス0の一部として行い、対象名前空間の識別子はパス0〜1b のスキャン結果を参照する。import 対象の名前空間が同じスコープ内に定義されていれば、記述順序に依存しない。

### Q2: import の import は伝播するか？

**A**: 伝播しない。A が B を import し、C が A を import した場合、C からは A に直接定義された識別子のみアクセスでき、B の識別子にはアクセスできない。

### Q3: import した識別子の種別をまたがる衝突はどうするか？

**A**: import 元に constexpr `n` があり、現在の名前空間に変数 `n` がある場合も衝突としてエラー（`weak:` なし）。種別を問わず同名の識別子は衝突する。
