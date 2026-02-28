# semantic_analyzer 分割設計

## 現状

[src/semantic_analyzer/](../../../src/semantic_analyzer/) は以下のファイルで構成:

| ファイル | 行数 | 役割 |
|----------|------|------|
| mod.rs | 1801 | メイン解析ロジック（全責務が混在） |
| scope.rs | 517 | スコープ管理・シンボル解決 |
| types.rs | — | 型定義 (ExecExpression, ExecStatement 等) |
| tests.rs | 760 | テスト |

`mod.rs` が 1801 行あり、7 つの異なる責務が単一ファイルに混在している。

## mod.rs の関数クラスタ分析

詳細な呼び出しグラフ分析の結果、以下の 7 クラスタを特定した。

### Cluster 1: Constexpr 評価 (L40–200, 160行)

| 関数 | 行範囲 | 概要 |
|------|--------|------|
| `evaluate_constexpr_expr` | L40–120 | 式の再帰評価 |
| `evaluate_constexpr_by_name` | L123–168 | 名前による遅延解決（巡回検知付き） |
| `collect_constexpr_table` | L174–200 | ステートメント列から constexpr テーブル構築 |

- **依存**: AST 型のみ + `base::constexpr_eval`
- **出力**: `BTreeMap<String, i64>`
- **分割容易度**: ⭐⭐⭐⭐⭐（完全に自己完結）

### Cluster 2: Alias 処理 (L205–414, 210行)

| 関数 | 行範囲 | 概要 |
|------|--------|------|
| `collect_alias_map` | L205–229 | 識別子エイリアス収集 |
| `collect_block_alias_map` | L234–271 | ブロックエイリアス収集 |
| `collect_block_alias_refs_in_stmts` | L273–283 | 参照収集 (stmts) |
| `collect_block_alias_refs_in_stmt` | L285–320 | 参照収集 (stmt) |
| `collect_block_alias_refs_in_expr` | L322–369 | 参照収集 (expr) |
| `detect_block_alias_cycles` | L374–414 | DFS 巡回検知（内部に `dfs` ネスト関数） |

- **依存**: AST 型のみ
- **出力**: `BTreeMap<String, String>`, `BTreeMap<String, Vec<LocatedStatement>>`
- **分割容易度**: ⭐⭐⭐⭐⭐（完全に自己完結）

### Cluster 3: Return 解析 (L419–497, 80行)

| 関数 | 行範囲 | 概要 |
|------|--------|------|
| `has_return_statement` | L419–449 | return 文の存在チェック |
| `expr_contains_return` | L452–461 | 式内 return チェック |
| `guarantees_return` | L468–479 | 全パスで return を保証するか |
| `expr_guarantees_return` | L482–497 | 式の return 保証チェック |

- **依存**: AST 型のみ（`LocatedStatement`, `Expression`）
- **出力**: `bool`（純粋関数）
- **分割容易度**: ⭐⭐⭐⭐⭐（完全に自己完結、副作用なし）

### Cluster 4: ヘルパーユーティリティ (L499–519, 20行)

| 関数 | 行範囲 | 概要 |
|------|--------|------|
| `require_int_type` | L499–508 | void 型チェック |
| `make_located_exec` | L511–519 | LocatedExecExpression コンストラクタ |

- **依存**: `types` モジュールの型
- **分割容易度**: ⭐⭐⭐⭐（小さい。Cluster 5 と一体化が自然）

### Cluster 5: 式変換 (L524–959, 435行)

| 関数 | 行範囲 | 概要 |
|------|--------|------|
| `convert_to_exec_expression_with_resolver` | L524–959 | 全 Expression バリアントを ExecExpression に変換する巨大 match |

内部セクション:
- L527–593: `Ref` 演算子 (`&`) の処理
- L594–604: 単項演算子
- L606–720: 二項演算子（複合代入展開、final 変数チェック含む）
- L721–770: `If` 式
- L771–789: `Block` 式
- L790–907: `Function` 呼び出し（組み込み関数、ブロックエイリアス展開、ユーザー関数）
- L908–920: リテラル・変数
- L921–957: 配列アクセス

- **依存**: `ScopeResolver` に強く依存。Cluster 7 と**相互再帰**
- **結合点**: `analyze_internal_with_parent` を If/Block/While 式から呼び出す
- **分割容易度**: ⭐⭐（相互再帰が障壁）

### Cluster 6: テンプレート展開 (L962–1169, 207行)

| 関数/型 | 行範囲 | 概要 |
|---------|--------|------|
| `TemplateEntry` (struct) | L962–966 | テンプレート定義情報 |
| `expand_template_instantiations` | L972–1169 | テンプレート関数のインスタンス化 |

- **依存**: AST 型のみ。`LocatedStatement → LocatedStatement` の純粋な変換
- **分割容易度**: ⭐⭐⭐⭐（自己完結度が高い）

### Cluster 7: コア解析エントリ (L1171–1801, 630行)

| 関数 | 行範囲 | 概要 |
|------|--------|------|
| `analyze_internal` | L1171–1183 | 内部ラッパー |
| `analyze_internal_with_parent` | L1188–1782 | 4パス解析の本体（594行） |
| `analyze` | L1784–1798 | 外部エントリポイント (pub) |

`analyze_internal_with_parent` の内部パス:
| パス | 行範囲 | 内容 |
|------|--------|------|
| プレパス | L1213–1215 | テンプレート展開 |
| Pass 0 | L1237–1245 | constexpr/alias/block_alias 収集・巡回チェック |
| Pass 1a | L1251–1308 | 関数宣言のホイスティング |
| 型決定 | L1315–1320 | `effective_func_return_types` の決定 |
| Pass 1b | L1323–1349 | 変数宣言のホイスティング |
| Scope構築 | L1353–1413 | `temporary_scope` 構築 → `ScopeResolver` 作成 |
| Pass 2 | L1416–1782 | 文の変換 (`Statement` → `ExecStatement`) |

- **依存**: 全 Cluster の出力を統合。Cluster 5 と相互再帰
- **可変状態**: `global_functions: &mut Vec<Function>`, `global_function_names: &mut Vec<String>`
- **分割容易度**: ⭐（最も困難。Cluster 5 との結合を解消する必要あり）

## 分割方針

### Phase 1: 自己完結クラスタの分離（容易）

Cluster 1, 2, 3, 6 は外部依存がなく、安全に分離可能。

```
src/semantic_analyzer/
├── mod.rs              # analyze() + Cluster 7 (コア解析)
├── scope.rs            # 既存（変更なし）
├── types.rs            # 既存（変更なし）
├── tests.rs            # 既存（変更なし）
├── constexpr.rs        # ← Cluster 1 (160行)
├── alias.rs            # ← Cluster 2 (210行)
├── return_analysis.rs  # ← Cluster 3 (80行)
└── template.rs         # ← Cluster 6 (207行)
```

**mod.rs の行数変化**: 1801 → ~1144 (657行削減, 36%減)

#### 各ファイルの公開インターフェース

**constexpr.rs**:
```rust
pub(super) fn collect_constexpr_table(
    statements: &[LocatedStatement],
) -> Result<BTreeMap<String, i64>, Vec<CodeParseError>>
```

**alias.rs**:
```rust
pub(super) fn collect_alias_map(
    statements: &[LocatedStatement],
) -> Result<BTreeMap<String, String>, Vec<CodeParseError>>

pub(super) fn collect_block_alias_map(
    statements: &[LocatedStatement],
    alias_map: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Vec<LocatedStatement>>, Vec<CodeParseError>>

pub(super) fn detect_block_alias_cycles(
    block_alias_map: &BTreeMap<String, Vec<LocatedStatement>>,
    alias_map: &BTreeMap<String, String>,
) -> Result<(), Vec<CodeParseError>>
```

**return_analysis.rs**:
```rust
pub(super) fn has_return_statement(statements: &[LocatedStatement]) -> bool
pub(super) fn guarantees_return(statements: &[LocatedStatement]) -> bool
```

**template.rs**:
```rust
pub(super) fn expand_template_instantiations(
    statements: &[LocatedStatement],
) -> Result<Vec<LocatedStatement>, Vec<CodeParseError>>
```

### Phase 2: 式変換の分離（中難度）

Cluster 5 (`convert_to_exec_expression_with_resolver`) を分離する。

**相互再帰の解決策**: 関数ポインタまたはコールバックを渡す。

```rust
// expression.rs
pub(super) fn convert_to_exec_expression_with_resolver(
    located_expr: &Box<LocatedExpression>,
    parent_resolver: &ScopeResolver,
    func_return_types: &[ValueType],
    // 相互再帰を解消するためのコールバック
    analyze_block: &dyn Fn(
        &Vec<LocatedStatement>,
        ScopeType,
        Vec<String>,
        &ScopeResolver,
        &mut Vec<Function>,
        &mut Vec<String>,
        Option<usize>,
        Vec<ValueType>,
    ) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>>,
) -> Result<Box<LocatedExecExpression>, Vec<CodeParseError>>
```

ただしこの方法は型定義が冗長になる。代替案:

**代替案 A — trait による抽象化**:
```rust
pub(super) trait BlockAnalyzer {
    fn analyze_block(
        &mut self,
        statements: &Vec<LocatedStatement>,
        scope_type: ScopeType,
        ...
    ) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>>;
}
```

**代替案 B — 同一ファイルを維持し関数のみ分離**:

Phase 1 の分離で mod.rs は 1144 行になり、十分に管理可能なサイズとなる。
Cluster 5 と 7 は相互再帰のため同一ファイルに留める判断も妥当。

### Phase 3: コンテキスト構造体の導入（任意）

`analyze_internal_with_parent` の 8 引数を構造体化:

```rust
pub(super) struct AnalyzeContext<'a> {
    pub scope_type: ScopeType,
    pub initial_vars: Vec<String>,
    pub parent_resolver: Option<&'a ScopeResolver<'a>>,
    pub global_functions: &'a mut Vec<Function>,
    pub global_function_names: &'a mut Vec<String>,
    pub func_global_index: Option<usize>,
    pub inherited_func_return_types: Vec<ValueType>,
}
```

Phase 2 の相互再帰問題にも活用可能。

## 推奨実行順序

1. **Phase 1** を先行実施 — リスクが低く、効果が大きい（657行削減）
2. Phase 3 のコンテキスト構造体を導入 — 可読性向上
3. Phase 2 は必要に応じて実施 — 相互再帰の解消コストと管理可能行数のトレードオフ

## テストへの影響

- Phase 1: `tests.rs` は `mod.rs` の公開インターフェース (`analyze`) 経由でテストしているため、内部分割の影響を受けない（テスト修正不要）
- Phase 2: 同上（`analyze` の動作は不変）
- Phase 3: 内部 API のシグネチャ変更だが、外部テストには影響なし

## リスク

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| 分割後のコンパイルエラー | 中 | `pub(super)` で段階的に公開範囲を制御 |
| テスト回帰 | 低 | 外部 API 不変のため `cargo test` で検証可能 |
| Phase 2 の相互再帰解消が複雑 | 高 | Phase 1 で十分なら Phase 2 は見送り |
