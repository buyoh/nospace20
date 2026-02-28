# Step 6: constexpr ブロック形式 設計メモ

## 概要

ブロック形式の constexpr 定義を実装する。
式形式の `constexpr: name(expr);` に加え、ブロック内の処理結果をコンパイル時定数として使用する。

**親ドキュメント**: [unimplemented-variable-features.md](unimplemented-variable-features.md) §2.4, §4.7

---

## 1. 構文

```nospace
constexpr: VALUE {
  let: tmp(3);
  tmp * tmp;   # = 9 #
};
```

ブロック内の全処理がコンパイル時に評価可能な場合のみ許可。
ブロックの最後の式の値がコンパイル時定数となる。

---

## 2. 実装の複雑さ

constexpr ブロック形式は、実質的に **コンパイル時インタプリタ** の構築を要求する。

現在の `evaluate_constexpr_expr()` は式レベルの評価のみ対応しており、
以下の要素をサポートする必要がある:

| 要素 | 対応難易度 | 備考 |
|------|----------|------|
| ローカル変数 (`let:`) | 中 | コンパイル時変数テーブルの管理が必要 |
| 代入 (`=`) | 中 | コンパイル時変数への代入 |
| 条件分岐 (`if:`) | 中〜高 | コンパイル時の条件評価と分岐 |
| ループ (`while:`, `for:`) | 高 | 無限ループ検知、反復上限が必要 |
| 関数呼び出し | 非対応 | constexpr ブロック内での関数呼び出しは禁止 |
| 配列 | 高 | コンパイル時配列の管理 |
| 参照・ポインタ | 非対応 | コンパイル時アドレスは無意味 |

### 2.1 最小限の実装案

初回実装では以下のみサポート:
- ローカル変数（`let:`）の宣言と代入
- 算術/比較/論理演算
- `if:` 条件分岐（ブロック形式）
- 他の constexpr 参照

**サポートしない**:
- ループ (`while:`, `for:`, `repeat:`)
- 関数呼び出し
- 配列
- 参照・ポインタ
- `break:`, `continue:`, `return:`

### 2.2 コンパイル時インタプリタの設計

```rust
/// コンパイル時環境（constexpr ブロック評価用）
struct ConstexprEnvironment {
    /// ローカル変数テーブル（名前 → 値）
    variables: BTreeMap<String, i64>,
    /// 外側の constexpr テーブル（既存）
    constexpr_table: &BTreeMap<String, i64>,
}

/// constexpr ブロックを評価する
fn evaluate_constexpr_block(
    statements: &[LocatedStatement],
    env: &mut ConstexprEnvironment,
) -> Result<i64, Vec<CodeParseError>> {
    let mut last_value: Option<i64> = None;
    for stmt in statements {
        match &stmt.statement {
            Statement::VariableDeclaration(name, init, _, _, _) => {
                let value = evaluate_constexpr_expr_in_env(&init.expression, env)?;
                env.variables.insert(name.clone(), value);
            }
            Statement::Expression(expr) => {
                last_value = Some(evaluate_constexpr_expr_in_env(&expr.expression, env)?);
            }
            _ => {
                return Err(vec![code_parse_error!(
                    stmt.location.start,
                    "unsupported statement in constexpr block"
                )]);
            }
        }
    }
    last_value.ok_or_else(|| vec![code_parse_error!("constexpr block has no value")])
}
```

---

## 3. パーサー変更

### 3.1 Statement enum

ブロック形式の constexpr は `ConstexprDeclaration` のバリアントを拡張するか、
新しいバリアントを追加する。

**方法A**: 新バリアント
```rust
ConstexprBlockDeclaration(String, Vec<LocatedStatement>) // (name, block)
```

**方法B**: ConstexprDeclaration の init_expr をブロック式にする
```rust
// constexpr: name { ... }; → ConstexprDeclaration(name, Block(stmts))
// 既存の Expression::Block を使用
```

**推奨**: 方法B。既存の `Expression::Block` を利用して `ConstexprDeclaration` の式部分をブロック式にする。
パーサーで `constexpr:` の後に `{` が来た場合、ブロック式をパースして `ConstexprDeclaration` に格納する。

### 3.2 パーサー分岐

```rust
// parse_constexpr_declarations() 内
// 識別子取得後の次トークンで分岐:
// '(' → 式形式（既存）
// '{' → ブロック形式（新規）
```

---

## 4. 意味解析変更

Pass 0 の `collect_constexpr_table()` で、ブロック形式の constexpr を評価する。

```rust
// 式がブロックの場合、evaluate_constexpr_block を使用
// それ以外は既存の evaluate_constexpr_expr を使用
```

---

## 5. 実装時期の判断

constexpr ブロック形式は以下の理由から **優先度が低い**:

1. コンパイル時インタプリタの実装コストが高い
2. 式形式 constexpr + 通常の定数計算で大部分のユースケースをカバー可能
3. テストの複雑さが増す

**推奨**: Step 4 (ブロックエイリアス) と Step 5 (final 変数) を先に実装し、
constexpr ブロック形式は需要が明確になった時点で実装する。

---

## 6. 変更ファイル一覧（予定）

| ファイル | 変更内容 |
|---------|---------|
| `src/tree_parser/statement/mod.rs` | `parse_constexpr_declarations` のブロック形式対応 |
| `src/semantic_analyzer/mod.rs` | `evaluate_constexpr_block()` 追加、Pass 0 でブロック評価対応 |
| `resources/tests/` | constexpr ブロックのテストケース追加 |
