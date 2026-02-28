# Step 4: ブロックエイリアス詳細設計

## 概要

ブロックエイリアス `alias: name { 文... };` を実装する。
識別子エイリアス（Step 3 実装済み）に加え、ブロック（文の列）を名前に紐付けて呼び出し時に展開する機能。

**親ドキュメント**: [unimplemented-variable-features.md](unimplemented-variable-features.md) §1.2, §1.3, §4.5

---

## 1. パーサー変更（tree_parser）

### 1.1 Statement enum 追加

```rust
// src/tree_parser/statement/mod.rs
pub enum Statement {
    // ... 既存 ...
    /// ブロックエイリアス定義: `alias: name { 文... };`
    /// コンパイル時にブロックを名前に紐付け、呼び出し時に AST をインライン展開する
    AliasBlock(String, Vec<LocatedStatement>), // (name, body)
    // ...
}
```

### 1.2 パーサー分岐の修正

現在の `parse_alias_declarations()` は `alias:` の後に必ず `name(target)` を期待する。
ブロックエイリアスの場合、`alias: name { ... };` 構文となるため、名前取得後のトークンで分岐が必要。

**変更箇所**: `src/tree_parser/statement/mod.rs` の `parse_alias_declarations()`

#### 識別子/ブロック判定ロジック

```
alias: name(target);      → AliasIdentifier（既存）
alias: name { ... };      → AliasBlock（新規）
alias: a(b), c(d);        → 複数 AliasIdentifier（既存）
```

名前トークンを消費した後の次トークンで判定：
- `(` → 識別子エイリアス（既存パス）
- `{` → ブロックエイリアス（新規パス）

#### 実装アプローチ

`parse_alias_declarations()` の主ループ内で、識別子名を取得した後のトークンを peek して分岐する。

```rust
// 名前を取得後
match self.iter.peek() {
    Some((Token::ParenthesisL, _)) => {
        // 既存の識別子エイリアスパース処理
        // ...
    }
    Some((Token::BraceL, _)) => {
        // ブロックエイリアス: alias: name { ... };
        // parse_to_block() 相当でブロック本体をパース
        let body = self.parse_block_body(); // 内部の文列をパース
        results.push(LocatedStatement {
            statement: Statement::AliasBlock(name.to_string(), body),
            location: loc,
        });
        // ブロックエイリアスの後は ';' を消費して終了（複数定義不可）
        break;
    }
    _ => {
        // エラー: expected '(' or '{' after alias identifier
    }
}
```

**注意**: ブロックエイリアスは複数定義（`,` 区切り）に対応しない。
`alias: a { ... }, b { ... };` は構文が複雑になるため、単一定義のみとする。

### 1.3 ブロック本体のパース

ブロックの中身は通常の文列をパースする。既存の `parse_to_statements()` はブレース `{}` の外側から呼ばれるため、
`{` を消費してから `}` が来るまで文をパースする `parse_block_body()` 相当の処理を使う。

既存の `parse_to_expression_block()` や内部のブロックパース処理を参考に、
`{` と `}` の間の文列を `Vec<LocatedStatement>` として返す。

---

## 2. 意味解析変更（semantic_analyzer）

### 2.1 Pass 0: ブロックエイリアスの収集

識別子エイリアスの `collect_alias_map()` と同様に、ブロックエイリアスを収集する関数を追加する。

```rust
/// ステートメント列からブロックエイリアス定義を収集し、
/// ブロックエイリアステーブル `BTreeMap<String, Vec<LocatedStatement>>` を返す。
fn collect_block_alias_map(
    statements: &[LocatedStatement],
) -> Result<BTreeMap<String, Vec<LocatedStatement>>, Vec<CodeParseError>> {
    let mut block_alias_map: BTreeMap<String, Vec<LocatedStatement>> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    for located_stat in statements {
        if let Statement::AliasBlock(name, body) = &located_stat.statement {
            if block_alias_map.contains_key(name) {
                errors.push(code_parse_error!(
                    located_stat.location.start,
                    format!("duplicate block alias definition: '{}'", name)
                ));
            } else {
                block_alias_map.insert(name.clone(), body.clone());
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(block_alias_map)
}
```

**名前衝突チェック**: 識別子エイリアスとブロックエイリアスで同名の定義がある場合はエラーとすべき。
`collect_alias_map()` と `collect_block_alias_map()` の結果をクロスチェックする。

```rust
// Pass 0 の後
let alias_map_temp = collect_alias_map(statements)?;
let block_alias_map_temp = collect_block_alias_map(statements)?;

// 名前衝突チェック
for name in block_alias_map_temp.keys() {
    if alias_map_temp.contains_key(name) {
        return Err(vec![code_parse_error!(
            format!("alias '{}' is defined as both identifier alias and block alias", name)
        )]);
    }
}
```

### 2.2 ScopeInfo への追加

```rust
// src/semantic_analyzer/scope.rs
pub(super) struct ScopeInfo<'a> {
    // ... 既存フィールド ...
    /// ブロックエイリアステーブル（名前 → AST 本体）
    pub block_alias_map: &'a BTreeMap<String, Vec<LocatedStatement>>,
}
```

### 2.3 enter_scope シグネチャ変更

```rust
pub fn enter_scope(
    &mut self,
    // ... 既存引数 ...
    alias_map: &'a BTreeMap<String, String>,
    block_alias_map: &'a BTreeMap<String, Vec<LocatedStatement>>,  // 追加
    is_function_scope: bool,
    func_global_index: Option<usize>,
)
```

**影響範囲**: `enter_scope()` の全呼び出し元（3箇所 + テスト）を更新する必要がある。
空の `BTreeMap` を渡すパターンは `for:` の init スコープと同様。

### 2.4 ScopeResolver へのメソッド追加

```rust
impl<'a> ScopeResolver<'a> {
    /// ブロックエイリアス名を解決し、AST 本体を返す
    ///
    /// スコープスタックを内側から外側へ探索する。
    /// 見つからない場合は None を返す。
    pub fn resolve_block_alias(&self, name: &str) -> Option<&Vec<LocatedStatement>> {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(body) = scope_info.block_alias_map.get(name) {
                return Some(body);
            }
        }
        None
    }
}
```

### 2.5 Expression::Function での展開

`convert_to_exec_expression_with_resolver()` 内の `Expression::Function` 処理を変更する。

現在の処理フロー:
```
1. 引数を変換
2. 組み込み関数チェック
3. alias チェーン解決
4. ユーザー定義関数として解決
```

変更後のフロー:
```
1. 組み込み関数チェック（引数変換なし、名前だけ）
2. alias チェーン解決
3. ブロックエイリアスチェック ← 新規
   - 引数が 0 でなければエラー
   - AST をクローンして Block 式として展開
4. 引数を変換（ブロックエイリアスでない場合のみ）
5. ユーザー定義関数として解決
```

**重要**: ブロックエイリアスの場合、引数は不要（0個でなければエラー）。
引数の変換はブロックエイリアスでないことが確認された後に行う。

ただし、現在のコードは引数変換を関数名判定より前に行っている。
ブロックエイリアス展開を挿入するためには、処理順序の変更が必要。

#### 処理順序の再構成案

```rust
Expression::Function(f, a) => {
    // 1. 組み込み関数チェック（名前のみで判定可能）
    let builtin_kind = match f.as_str() {
        "__puti" => Some(BuiltinFunctionKind::Puti),
        // ...
        _ => None,
    };

    if let Some(kind) = builtin_kind {
        // 組み込み関数: 引数変換 + 引数数チェック
        let mut args = Vec::new();
        for e in a {
            let exec_arg = convert_to_exec_expression_with_resolver(e, parent_resolver, func_return_types)?;
            require_int_type(&exec_arg, func_return_types)?;
            args.push(exec_arg);
        }
        // ... 引数数チェック + BuiltinFunction 生成 ...
    } else {
        // 2. alias チェーン解決
        let resolved_f = parent_resolver.resolve_alias_chain(f).map_err(|e| {
            vec![code_parse_error!(loc.start, e)]
        })?;

        // 3. ブロックエイリアスチェック
        if let Some(block_body) = parent_resolver.resolve_block_alias(&resolved_f) {
            // ブロックエイリアス展開
            if !a.is_empty() {
                return Err(vec![code_parse_error!(
                    loc.start,
                    format!("block alias '{}' does not accept arguments", f)
                )]);
            }
            // 巡回検知（§2.6 参照）
            // ...

            // AST をクローンして analyze_internal_with_parent で展開
            let cloned_body = block_body.clone();
            let (s, es) = analyze_internal_with_parent(
                &cloned_body,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver), // 呼び出し元のスコープで名前解決
                &mut Vec::new(),
                &mut Vec::new(),
                None,
                func_return_types.to_vec(),
            )?;
            Ok(make_located_exec(
                ExecExpression::Block(Block {
                    scope: s.build(Vec::new(), Vec::new(), Vec::new()),
                    statements: es,
                }),
                loc,
            ))
        } else {
            // 4. ユーザー定義関数（既存処理）
            let mut args = Vec::new();
            for e in a {
                let exec_arg = convert_to_exec_expression_with_resolver(
                    e, parent_resolver, func_return_types,
                )?;
                require_int_type(&exec_arg, func_return_types)?;
                args.push(exec_arg);
            }
            let func_ref = parent_resolver.resolve_function(&resolved_f).ok_or_else(|| {
                vec![code_parse_error!(loc.start, format!("undefined function: {}", f))]
            })?;
            // ... 引数数チェック + UserFunction 生成 ...
        }
    }
}
```

### 2.6 巡回参照の検知

ブロックエイリアスの巡回展開を検知するために、**展開スタック**を管理する。

#### アプローチ: 静的な依存グラフ解析（推奨）

Pass 0 でブロックエイリアスの AST を走査し、各ブロックエイリアスが参照する他のブロックエイリアスの依存グラフを構築する。
グラフに巡回があればコンパイルエラー。

**利点**:
- `convert_to_exec_expression_with_resolver` のシグネチャ変更が不要
- 展開前に事前チェックできる
- 実装がシンプル

**欠点**:
- スコープをまたぐブロックエイリアスの参照は検出が複雑（同一スコープ内のみチェック）
- 動的な名前解決（alias チェーン経由でブロックエイリアスに到達するケース）の検出が困難

```rust
/// ブロックエイリアスの巡回参照を検知する
///
/// block_alias_map 内の各ブロックの AST を走査し、
/// 他のブロックエイリアスへの呼び出しを検出して依存グラフを構築する。
/// DFS で巡回を検知する。
fn detect_block_alias_cycles(
    block_alias_map: &BTreeMap<String, Vec<LocatedStatement>>,
    alias_map: &BTreeMap<String, String>,
) -> Result<(), Vec<CodeParseError>> {
    // AST 内の Expression::Function(name, []) を再帰的に探索
    // name が alias_map 経由で解決された後 block_alias_map に存在するか確認
    // 依存関係を建設 → DFS で巡回検知
    // ...
}
```

#### アプローチ B: 動的な展開スタック（代替案）

`convert_to_exec_expression_with_resolver` に `expanding_block_aliases: &mut Vec<String>` パラメータを追加する。

**利点**:
- 完全な巡回検知が可能（スコープ横断、alias チェーン経由も含む）

**欠点**:
- `convert_to_exec_expression_with_resolver` と関連する全関数のシグネチャ変更が必要
- コード変更範囲が大きい

#### 推奨: 静的解析 + 展開深度制限

- Pass 0 で同一スコープ内の明確な巡回を静的に検出
- 展開時に深度カウンタを設け、上限（例: 256）を超えた場合にエラー
  - 深度カウンタは `analyze_internal_with_parent` への追加パラメータとして渡す
  - または `ScopeResolver` にカウンタを保持する

---

## 3. Pass 1b での扱い

ブロックエイリアスは変数スロットを確保しない。Pass 1b では無視する。

```rust
// パス1b: 変数宣言収集
Statement::AliasBlock(_, _) => {
    // ブロックエイリアスはシンボルテーブルに登録しない - パス0 で処理済み
}
```

---

## 4. テストケース

### 成功ケース

#### block_alias_basic_001
```nospace
func: __main() {
  let: x(10);
  alias: print_x {
    __puti(x);
  };
  print_x();
}
```
期待出力: `10`

#### block_alias_scope_001
呼び出し元スコープで名前解決されることを確認。
```nospace
func: __main() {
  let: x(5);
  alias: inc_x {
    x = x + 1;
  };
  inc_x();
  __assert(x == 6);
  __trace(0);
}
```

#### block_alias_return_value_001
ブロックエイリアスの最後の式が値を返すことを確認。
```nospace
func: __main() {
  let: x(3);
  alias: square_x {
    x * x;
  };
  let: result(square_x());
  __assert(result == 9);
  __trace(0);
}
```

#### block_alias_nested_001
ブロックエイリアス内から別のブロックエイリアスを呼び出す。
```nospace
func: __main() {
  let: x(0);
  alias: inc { x = x + 1; };
  alias: inc3 {
    inc();
    inc();
    inc();
  };
  inc3();
  __assert(x == 3);
  __trace(0);
}
```

### エラーケース

#### block_alias_circular_001
巡回参照でコンパイルエラー。
```nospace
func: __main() {
  alias: a { b(); };
  alias: b { a(); };
  a();
}
```
期待: コンパイルエラー

#### block_alias_with_args_001
引数付き呼び出しでコンパイルエラー。
```nospace
func: __main() {
  alias: greet { __puti(42); };
  greet(1);
}
```
期待: コンパイルエラー（block alias does not accept arguments）

---

## 5. interpreter / compiler_ws への影響

変更不要。ブロックエイリアスは意味解析段階で `ExecExpression::Block` に展開されるため、
後段のインタプリタとコンパイラには透過的。

---

## 6. 未決定事項

1. **ブロックエイリアスの複数定義**: `alias: a { ... }, b { ... };` を許可するか？
   → 初回実装では単一定義のみ。

2. **識別子エイリアスとブロックエイリアスの名前空間**:
   同名の識別子エイリアスとブロックエイリアスを同一スコープで定義した場合の動作。
   → 同名定義はコンパイルエラーとする。

3. **alias チェーン経由のブロックエイリアス解決**:
   `alias: a(b); alias: b { ... };` の場合、`a()` でブロックエイリアス `b` が展開されるか？
   → 解決チェーンで最終名が得られた後、ブロックエイリアスもチェックするため、動作する。

4. **展開深度制限**: 再帰的展開の深度上限値。
   → 256 程度で十分と考えられる。

---

## 7. 変更ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `src/tree_parser/statement/mod.rs` | `Statement::AliasBlock` 追加、パーサー分岐修正 |
| `src/semantic_analyzer/scope.rs` | `ScopeInfo::block_alias_map` 追加、`enter_scope` 引数追加、`resolve_block_alias()` 追加 |
| `src/semantic_analyzer/mod.rs` | `collect_block_alias_map()` 追加、Pass 0 に組み込み、Expression::Function での展開ロジック追加、巡回検知 |
| `resources/tests/passes/alias/` | テストケース追加 |
