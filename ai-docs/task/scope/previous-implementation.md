# 過去の実装コミット（7a83612）の分析

## コミット情報

- **コミット**: 7a83612da56d4bcd9f0d16c564f42979b8de951b
- **ブランチ**: prv_wip
- **タイトル**: [WIP] Add PendingScope
- **日付**: 2022-01-01
- **状態**: revert 済み

## 変更概要

| ファイル | 変更内容 |
|----------|----------|
| `src/syntactic_analyzer/mod.rs` | 大幅リファクタリング（345行→168行変更） |
| `src/syntactic_analyzer/pending_scope.rs` | 新規追加（262行） |
| `src/syntactic_analyzer/pending_exectree.rs` | 新規追加（32行） |
| `src/syntactic_analyzer/scope.rs` | 新規追加（68行） |
| `src/syntactic_analyzer/exectree.rs` | 新規追加（20行） |
| `src/interpreter/mod.rs` | 変更（55行） |
| `resources/test/c004.ns` | テスト追加（ブロック内変数） |

## アーキテクチャ

### 2パス方式

1. **第1パス**: 構文木 → PendingScope/PendingExecStatement
   - 識別子は文字列のまま保持
   - スコープ構造を構築

2. **第2パス**: PendingScope → Scope（解決済み）
   - `ScopeStackResolver` を使用して識別子を解決
   - 文字列識別子を `Identifier { scope, local }` に変換

### 主要なデータ構造

#### Identifier

```rust
#[derive(PartialEq, PartialOrd, Eq, Ord, Clone)]
pub struct Identifier {
    scope: usize,   // どのスコープに属するか
    local: usize,   // スコープ内でのインデックス
}
```

#### Block

```rust
pub struct Block {
    pub scope: Scope,
    pub code: Vec<ExecStatement>,
}
```

#### ScopeType

```rust
pub enum ScopeType {
    Global,     // グローバルスコープ
    Function,   // 関数スコープ
    Block,      // ブロックスコープ（if/while）
}
```

#### Entity（識別子が指すもの）

```rust
pub enum Entity {
    Variable(Variable),
    Function(Function),
}
```

### PendingScope と ScopeStackResolver

```rust
pub struct PendingScope {
    scope_identifier: usize,
    scope_type: ScopeType,
    identifier_map: BTreeMap<Identifier, PendingEntity>,
    scope_dictionary: BTreeMap<String, Identifier>,
}

struct ScopeStackResolver<'a>(Vec<&'a PendingScope>);

impl<'a> ScopeStackResolver<'a> {
    fn resolve(&self, id_str: &String) -> Result<(Identifier, &PendingEntity), &str> {
        // スコープスタックを逆順にたどって識別子を解決
        // 関数スコープを超える変数アクセスはエラー
    }
}
```

## 識別子解決のルール

1. スコープスタックを末尾（現在のスコープ）から探索
2. 変数が関数スコープを超える場合はエラー
3. 関数は関数スコープを超えてアクセス可能（ホイスティング相当）

```rust
fn resolve(&self, id_str: &String) -> Result<(Identifier, &PendingEntity), &str> {
    let mut out_of_func = false;
    // スコープを逆順にたどる
    for scope in self.0.iter().rev() {
        if let Some(id) = scope.scope_dictionary.get(id_str) {
            let entity = scope.identifier_map.get(id).unwrap();
            match entity {
                PendingEntity::Function(_) => return Ok((id.clone(), entity)),
                PendingEntity::Variable(_) => {
                    if out_of_func {
                        return Err("cannot access variables over function scope");
                    }
                    return Ok((id.clone(), entity));
                }
            }
        }
        // 関数スコープを超えたらフラグを立てる
        if scope.scope_type != ScopeType::Block {
            out_of_func = true;
        }
    }
    Err("unknown identifier")
}
```

## インタプリタの変更

### LocalEnvironment の変更

```rust
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    current_scopes: Vec<&'a Scope>,      // スコープスタック
    parent_scope: Option<&'a Scope>,      // 親スコープ情報
    parent_env: Option<&'a LocalEnvironment<'a, 'aenv>>,
    variables: BTreeMap<Identifier, i64>, // 識別子→値のマップ
}
```

### 変数アクセス

```rust
fn get_variable(&self, id: &Identifier) -> Option<&mut i64> {
    if let Some(a) = self.variables.get_mut(id) {
        Some(a)
    } else if let Some(env) = self.parent_env {
        env.get_variable(id)
    } else {
        None
    }
}
```

## テストケース（c004.ns）

```nospace
func: main() {
  __trace(0);
  let:x;
  x=1;
  __assert(x == 1);
  if:1{
    let:x;     # シャドウイング #
    let:y;
    x=3;
    y=2;
    __assert(y == 2);
    __assert(x == 3);
  };
  __assert(x == 1);  # 外側の x は変更されていない #
}
```

## 複雑さと問題点

### 問題 1: ライフタイムの複雑さ

`ScopeStackResolver` と `LocalEnvironment` のライフタイム管理が複雑で、コンパイルエラーの原因になりやすい。

### 問題 2: 2パス方式のオーバーヘッド

すべての構造体が Pending 版と解決済み版の2種類必要で、コード量が増大。

### 問題 3: 未完成

- `get_variable` の実装にコメント「TODO: この実装は間違い」
- 親スコープ参照のロジックが不完全

## 簡略化の方向性

### Phase 1 での簡略化

1. **識別子は文字列のまま**: 事前解決は行わない
2. **実行時にスコープスタック探索**: シンプルだが効率は落ちる
3. **Block 構造体は導入**: スコープ情報を保持

### Phase 2 で識別子解決を追加

過去の実装を参考に、ただしよりシンプルな形で実装。

## 参考になる設計思想

1. **Block 構造体**: if/while がスコープを持つ概念は維持
2. **ScopeType 列挙型**: Global/Function/Block の区別
3. **識別子解決時のスコープ超えチェック**: 関数スコープを超える変数アクセスの禁止

## 使用可能なコード断片

以下は再利用可能:

```rust
pub enum ScopeType {
    Root,      // または Global
    Function,
    Block,
}

pub struct Block {
    pub scope: Scope,
    pub code: Vec<ExecStatement>,
}
```
