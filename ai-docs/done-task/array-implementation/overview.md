# 配列実装: 全体設計概要

## 仕様（spec.md §4.2, §4.3）

### 配列

```
let: arr[4];                   # サイズ4の配列を宣言
arr[0] = 1;                    # 要素へのアクセス
let: arr2[3](10, 20, 30);      # 初期値付き配列宣言
```

- 配列サイズは定数のみ指定可能
- `arr[i]` で i 番目の要素を参照（0始まり）
- `arr` 単体は `arr[0]` と同義
- `&arr` は配列の先頭要素の参照を取得

### 文字列

```
let: str1("Hello");            # サイズ6の配列（ヌル文字終端）
let: str2[6]('H', 'e', 'l', 'l', 'o', '\0');  # 同等
```

## 設計方針

### 1. フラットスロット方式

配列を「連続する複数の変数スロット」として表現する。

```
let: a;          # 1スロット (index 0)
let: arr[3];     # 3スロット (index 1, 2, 3)
let: b;          # 1スロット (index 4)
```

**利点:**
- 既存の `Vec<i64>` ベースの変数ストレージをそのまま活用
- `scope_stack[scope_idx][base_index + offset]` でアクセス可能
- 参照 (`&arr`) は既存のアドレス体系と自然に統合
- Whitespace コンパイラのメモリレイアウトとも整合

**制約:**
- 配列サイズはコンパイル時定数のみ（動的サイズ不可 = spec 通り）
- 境界チェックはランタイムで行う（配列サイズ情報を `Variable` に保持）

### 2. インデックスアクセスは postfix 演算として実装

`arr[i]` は `Variable("arr")` に対する postfix 操作。
構文木では新しいノード `ArrayAccess` として表現する。

### 3. 左辺値としてのインデックス

`arr[i] = x;` では `ArrayAccess` が代入の左辺値として機能する。
既存の `Variable` や `Deref` と同様に代入処理で特別扱いする。

### 4. 参照との統合

| 操作 | 意味 |
|------|------|
| `arr` | `arr[0]` の値 |
| `arr[i]` | 配列の i 番目の値 |
| `&arr` | `arr[0]` のアドレス |
| `&arr[i]` | `arr[i]` のアドレス（ = `&arr + i`） |
| `*(&arr + i)` | `arr[i]` と同義 |

## フェーズ分割

### Phase 1: 構文解析 (tree_parser)

- `Statement::VariableDeclaration` に配列サイズ情報を追加
- `Expression::ArrayAccess` バリアントを追加
- `let: arr[N];` のパースを実装
- `let: arr[N](vals...);` のパースを実装
- `arr[expr]` のパースを postfix として実装

### Phase 2: 意味解析 (semantic_analyzer)

- `Variable` に `array_size: Option<usize>` を追加
- `variable_count` の計算で配列サイズを考慮
- `ExecExpression::ArrayAccess` を追加
- `&arr[i]` の意味解析を実装
- バリデーション（配列サイズが定数か、重複定義チェック等）

### Phase 3: インタプリタ (interpreter)

- 配列アクセス（読み取り / 代入）の実行ロジック
- 配列初期化の実装
- `arr` 単体を `arr[0]` と同義に処理
- `&arr[i]` → アドレス計算
- 境界チェック

### Phase 4: Whitespace コンパイラ (compiler_ws)

- `allocate_global` をサイズ対応に変更
- 配列アクセスのコード生成
- ローカル配列のメモリレイアウト対応

### Phase 5: 文字列リテラル

- `let: str("Hello");` の構文パース（tree_parser レベルでの糖衣構文展開）
- 文字列を文字リテラル配列 + ヌル文字終端に変換

## データフロー

```
ソースコード: let: arr[3](10, 20, 30);
    ↓
Token Parser: [Keyword(Let), Colon, Identifier("arr"), BracketL, Number(3), BracketR,
               ParenthesisL, Number(10), Comma, Number(20), Comma, Number(30), ParenthesisR, Semicolon]
    ↓
Tree Parser: Statement::VariableDeclaration("arr", init_expr, false, Some(3))
             ※ init_expr は配列初期化用の特殊ノード
    ↓
Semantic Analyzer: Variable { identifier: "arr", is_static: false, array_size: Some(3) }
                   variable_count += 3  (3スロット占有)
                   ExecStatement::Expression(arr[0]=10, arr[1]=20, arr[2]=30)
    ↓
Interpreter: scope_stack[scope_idx] = [..., 10, 20, 30, ...]
```

## 影響範囲

| モジュール | 変更の程度 | 説明 |
|-----------|------------|------|
| token_parser | なし | `BracketL`/`BracketR` は既存 |
| tree_parser/statement | 中 | `VariableDeclaration` 拡張、配列パース追加 |
| tree_parser/expression | 中 | `ArrayAccess` バリアント追加、postfix パース |
| semantic_analyzer | 大 | `Variable` 拡張、スロット計算変更、`ExecExpression` 拡張 |
| interpreter/exec | 中 | 配列アクセス/代入、境界チェック |
| compiler_ws | 中 | メモリレイアウト・コード生成の配列対応 |

## 設計上の判断ポイント

### Q1: `arr` 単体の扱い

spec では「`arr` 単体は `arr[0]` と同義」。

**方針**: 意味解析時に `Variable("arr")` を `ArrayAccess(Variable("arr"), Factor(0))` に**変換しない**。
代わりに、インタプリタ/コンパイラが `Variable` を評価する際に、その変数が配列かどうかを `IdentifierRef` 経由で判断し、
配列の場合は先頭要素の値を返す。

理由: 純粋な `Variable` のままにすることで、代入 `arr = 5` が `arr[0] = 5` として動作し、
`&arr` が自然に先頭アドレスを返す。

**具体的実装**: `IdentifierRef` にフラグは追加しない。変数テーブルにある `array_size` 情報を参照して判断する。
`Variable(IdentifierRef)` の評価時、その `local_index` が配列の先頭を指しているだけであり、
既存の `get_variable` / `set_variable` がそのまま動作する（先頭スロットを読み書きする）。

### Q2: `IdentifierRef` への配列情報追加

**方針**: `IdentifierRef` は拡張しない。配列情報は `Variable` 構造体に保持。
`ExecExpression::ArrayAccess` は `IdentifierRef` + オフセット式を持つ。

理由: `IdentifierRef` は軽量な参照（Copy 可能な3フィールド）として設計されており、
配列サイズ情報は実行時にバリデーション（境界チェック）で必要だが、
頻繁にコピーされる `IdentifierRef` に含めるべきではない。

### Q3: 配列初期化式の表現

`let: arr[3](10, 20, 30);` の初期化は、以下のいずれか:

**案A**: Tree Parser レベルで複数の代入文に展開
```
arr[0] = 10; arr[1] = 20; arr[2] = 30;
```

**案B**: 専用の `ArrayInit` 式ノードを追加

**方針**: 案A を採用。tree_parser で `VariableDeclaration` の処理時に複数の代入 `Statement::Expression` を生成する。
理由: 既存の仕組みで自然に処理でき、専用ノードの追加が不要。

### Q4: 境界チェック

**方針**: インタプリタではランタイム境界チェックを行い、範囲外アクセスでパニックする。
Whitespace コンパイラでは境界チェックをスキップ（パフォーマンス優先）。

### Q5: 配列の `&` 演算子

`&arr` → `arr[0]` のアドレス（既存の `resolve_address` で対応可能）。
`&arr[i]` → 新しい Expression ノード。意味解析で `Ref(ArrayAccess(...))` として処理。

**方針**: `Ref(ArrayAccess(id_ref, offset))` をインタプリタで特別処理し、
`resolve_address(id_ref) + offset` を返す。
