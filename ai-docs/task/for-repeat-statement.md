# for 文・repeat 文の設計

## 概要

nospace 言語に `for` 文と `repeat` 文を追加する。

- `for` : 4つのブロック（初期化・条件・更新・本体）からなる汎用ループ文
- `repeat` : `for` の糖衣構文。カウンタ付きループを簡潔に記述

両方とも **文** (statement) であり、式としては使用できない（`while` と同様）。

## 構文仕様

### for 文

```bnf
for_stmt ::= "for" ":" block block block block ";"
```

4つのブロックをカンマなしで連続して記述する。

```
for: { 初期化; } { 条件; } { 更新; } { 本体; };
```

例:

```
for: { let: i(0); } { i < 5; } { i += 1; } { __puti(i); };
```

#### 構文の根拠

既存の制御構文との一貫性に基づく:

- `while: expr block ;` — パーツ間にカンマなし
- `if: expr block else: block ;` — パーツ間にカンマなし
- `,` は「同種のもの列挙」（`let` の複数宣言、関数引数）に使用される

`for` の4パーツは全てブロックであり、パーツ列挙ではないため、カンマなしが自然。パーサーは `for:` の後にブロック4つを期待するだけで曖昧性はない。

#### 各ブロックの役割

| ブロック | 役割 | 値の扱い |
|----------|------|----------|
| 初期化 | 変数宣言・初期値設定 | 値は破棄 |
| 条件 | ループ継続条件の評価 | 最後の式の値が条件値（int 必須） |
| 更新 | カウンタ更新等 | 値は破棄 |
| 本体 | ループごとの処理 | 値は破棄 |

### repeat 文

```bnf
repeat_stmt ::=
    | "repeat" ":" repeat_init "," expr "," expr ";"   # Form 1: カウンタ+回数+本体
    | "repeat" ":" repeat_init "," expr ";"            # Form 2: カウンタ+本体 (無限)
    | "repeat" ":" expr ";"                            # Form 3: 本体のみ (無限)

repeat_init ::= ident "(" expr ")"
```

`repeat` の本体は **式** である（ブロックスコープ式 `{ ... }` も使用可能）。

```
# Form 1: i を 0 から 4 まで繰り返す (5回)
repeat: i(0), 5, __puti(i);

# Form 1: ブロックスコープ式を使用
repeat: i(0), 5, {
    __puti(i);
};

# Form 2: i を 0 から無限に繰り返す
repeat: i(0), {
    __puti(i);
    if: i > 100 { break; };
};

# Form 3: 無限ループ（カウンタなし）
repeat: {
    __putc('.');
};
```

#### パースの曖昧性の解消

`i(0)` は関数呼び出し `Expression::Function("i", [Factor(0)])` と構文的に同一である。パーサーは以下の戦略で解消する:

1. `repeat:` の後、式をパースする
2. 次のトークンを確認:
   - `;` → Form 3（パースした式が本体）
   - `,` → パースした式を初期化宣言として再解釈
     - `Expression::Function(name, [init_val])` であれば `name(init_val)` として扱う
     - それ以外はエラー
3. 初期化宣言の後、次の式をパースし、次のトークンを確認:
   - `;` → Form 2
   - `,` → Form 1（パースした式がループ回数、もう一つ式をパースして本体）

## 意味論

### スコープ

`for` 文は初期化ブロック用の「for スコープ」を作成する。このスコープは条件・更新・本体の全ブロックの親スコープとなる。

```
for: { let: i(0); } { i < 5; } { i += 1; } { __puti(i); };
#     └── for スコープ ─────────────────────────────────────┘
#                        └ cond scope ┘ └ step scope ┘ └ body scope ┘
```

- 初期化ブロックで宣言された変数は、for スコープに所属する
- 条件・更新・本体ブロックはそれぞれ独自の子スコープを持つが、for スコープの変数にアクセス可能
- for スコープ内の変数は、for 文の終了時に破棄される

### break / continue

- `break`: 最も内側のループ（for / while）を抜ける
- `continue`: **本体の残りをスキップし、更新ブロックを実行してから条件を再評価する**

これが `while` との重要な違いであり、`for` が `while` の単純な脱糖ではない理由である。

```
for: { let: i(0); } { i < 5; } { i += 1; } {
    if: i == 2 { continue; };   # i += 1 は実行される #
    __puti(i);
};
# 出力: 0 1 3 4
```

### 型

- `for` 文は void 型（値を返さない）
- 条件ブロックの最後の式は int 型でなければならない
- 空の条件ブロック `{}` はエラー（void 型の条件は不可）

### repeat → for 脱糖

`repeat` は tree_parser 段階で `Statement::For` に変換される。`Statement::Repeat` は存在しない。

#### Form 1: `repeat: i(init), N, body;`

```
for: { let: i(init); let: __rpt_n(N); } { __rpt_n > 0; } { i += 1; __rpt_n -= 1; } { body; };
```

隠し変数 `__rpt_n` はカウントダウン用。`__` で始まるため予約識別子であり、ユーザーコードとの衝突は事実上ない。ネストされた repeat での衝突を避けるため、連番サフィックスを付与する（`__rpt_n0`, `__rpt_n1`, ...）。

#### Form 2: `repeat: i(init), body;`

```
for: { let: i(init); } { 1; } { i += 1; } { body; };
```

条件は常に真（定数 1）。無限ループ。

#### Form 3: `repeat: body;`

```
for: {} { 1; } {} { body; };
```

条件は常に真。初期化・更新なし。

## 内部表現

### tree_parser: Statement

```rust
// 新規追加
Statement::For(
    Vec<LocatedStatement>,     // init block statements
    Vec<LocatedStatement>,     // cond block statements
    Vec<LocatedStatement>,     // step block statements
    Vec<LocatedStatement>,     // body block statements
)
```

`repeat` は tree_parser で `Statement::For` に脱糖されるため、専用の variant は不要。

### token_parser: Keyword

```rust
pub enum Keyword {
    Let, Func, If, Else,
    While,
    For,      // 新規追加
    Repeat,   // 新規追加
    Return, Break, Continue, Static,
}
```

### semantic_analyzer: ExecStatement

```rust
// 新規追加
ExecStatement::For(
    Block,          // init: for スコープ + 初期化文
    ConditionMode,  // 条件モード（初期値は NonZero、optimizer が変換可能）
    Block,          // cond: 条件ブロック（最後の式の値が条件値）
    Block,          // step: 更新ブロック
    Block,          // body: 本体ブロック
)
```

`ConditionMode` は `While` と共用（`NonZero`, `Zero`, `Negative`）。意味解析では常に `NonZero` で生成し、optimizer が最適化する。

### スコープ構造

```
ExecStatement::For(init, mode, cond, step, body)
```

- `init.scope` は for スコープ（cond/step/body の変数解決の親）
- `cond.scope`, `step.scope`, `body.scope` は for スコープの子スコープ

## 実装計画

### Step 1: token_parser

- `Keyword` enum に `For`, `Repeat` を追加
- 文字列 → キーワード変換に `"for"`, `"repeat"` のマッピングを追加

### Step 2: tree_parser

- `Statement` enum に `For` variant を追加
- `for` 文のパースロジックを追加
  - `for:` → ブロック4つ → `;`
- `repeat` 文のパースロジックを追加
  - 式のパース → `,`/`;` による形式判定 → `Statement::For` への脱糖
- パース中の隠し変数名生成（`__rpt_n{counter}`）

### Step 3: semantic_analyzer

- `ExecStatement` enum に `For` variant を追加
- `Statement::For` の意味解析処理を追加:
  - for スコープの作成
  - init ブロックの解析（for スコープ内）
  - cond ブロックの解析（for スコープの子スコープ）
    - 最後の式が int 型であることを確認
  - step ブロックの解析（for スコープの子スコープ）
  - body ブロックの解析（for スコープの子スコープ）
  - ルートスコープでは使用不可（while と同様）

### Step 4: interpreter

- `interpret_for_statement` を追加:
  1. for スコープに入る（`enter_block(&init.scope)`）
  2. 初期化文を実行
  3. ループ:
     a. cond スコープに入り、条件文を実行、値を取得、スコープを出る
     b. ConditionMode に基づき判定。偽なら break
     c. body スコープに入り、本体を実行
     d. `Flow::Continue` または `Flow::Proceed` → body スコープを出る → step を実行
     e. `Flow::Break` → body スコープを出る → ループ終了
     f. `Flow::Return` → スコープを出て return
  4. for スコープを出る

### Step 5: compiler_ws

- `generate_for_statement` を追加
- `continue` ラベル処理の修正:
  - 現在: `push_loop_labels(loop_start, loop_end)` — continue は loop_start へジャンプ
  - 変更: `push_loop_labels(continue_target, break_target)` に名称変更
    - `while`: `continue_target = loop_start` （動作変更なし）
    - `for`: `continue_target = step_label` （更新ブロックの先頭）

コード生成パターン:

```
; init scope の変数を確保
; init 文を実行
loop_start:
  ; cond block を生成（スコープ確保→文実行→値がスタック上に残る）
  ; JumpIfZero(loop_end)         # ConditionMode::NonZero の場合
  ; body block を生成（スコープ確保→文実行→Discard）
continue_target:
  ; step block を生成（スコープ確保→文実行→Discard）
  ; Jump(loop_start)
loop_end:
  ; init scope の変数を解放
```

- `count_nested_vars_in_statement` に For の処理を追加

### Step 6: optimizer

全最適化パスに `ExecStatement::For` の match 分岐を追加:

| パス | 処理内容 |
|------|----------|
| condition_opt | cond ブロックの最後の式に対して ConditionMode 最適化を適用 |
| constant_folding | init/cond/step/body 各ブロック内の定数畳み込み。cond が定数偽なら body をクリア |
| dead_code | init/cond/step/body から到達可能関数を収集 |
| geti_opt | init/cond/step/body を再帰的に最適化 |

### Step 7: テスト

#### テストケース案

| テスト | 内容 |
|--------|------|
| for 基本 | `for: {let: i(0);} {i<5;} {i+=1;} {__puti(i);};` → 出力 01234 |
| for 空ブロック | `for: {} {1;} {} {};` → 無限ループ（break で抜ける） |
| for continue | continue 時に step が実行されることを確認 |
| for break | break でループを抜けることを確認 |
| for スコープ | init で宣言した変数が cond/step/body から参照可能 |
| for ネスト | for の中に for をネスト |
| repeat Form 1 | `repeat: i(0), 5, __trace(1);` → trace_hit_counts: {1: 5} |
| repeat Form 2 | 無限ループ + break |
| repeat Form 3 | 無限ループ + break |
| repeat ネスト | repeat の中に repeat |
| エラー: cond void | 条件ブロックが void → コンパイルエラー |
| エラー: グローバル使用 | トップレベルの for/repeat → コンパイルエラー |

#### ドキュメント更新

- `docs/spec.md` の for/repeat TODO を埋める
- `docs/grammar.bnf` に for_stmt / repeat_stmt を追加

## 設計上の判断まとめ

| 判断項目 | 決定 | 理由 |
|----------|------|------|
| for のブロック区切り | カンマなし | while/if との一貫性。`,` は同種列挙の意味 |
| for/repeat の種別 | 文（式ではない） | void を返す。関数引数のカンマと曖昧性回避 |
| repeat の内部表現 | For に脱糖 | tree_parser で変換。downstream の変更が不要 |
| 脱糖タイミング | tree_parser | 意味解析の変更を最小化。変数解決を自然に処理 |
| continue の行き先 | step ブロック | for の主要な存在意義。while との差別化 |
| ExecStatement 追加 | For のみ | repeat は脱糖済み |
| ConditionMode | While と共用 | NonZero/Zero/Negative。optimizer で最適化 |
