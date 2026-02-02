# スコープ実装の現状分析と概要

## 1. 現状のスコープ対応状況

### 1.1 言語仕様（spec.md セクション 7）が定義するスコープ

| 機能 | 仕様 | 現状 |
|------|------|------|
| スコープの親子関係 | 子から親へアクセス可能、逆は不可 | ❌ 未実装 |
| 関数スコープ | 関数ごとに独立したスコープ | ✅ 実装済（ただしテスト失敗中） |
| ブロックスコープ | if/while 内で独立したスコープ | ❌ 未実装（panic） |
| ブロック内変数定義 | let: をブロック内で使用可能 | ❌ 未実装（panic） |
| グローバルスコープ | 関数外での変数定義 | ❌ 未実装（panic） |
| static変数 | 親の関数スコープにアクセス可能 | ❌ 未実装 |

### 1.2 現状のコードパス

現在 `semantic_analyzer/mod.rs` では以下の箇所で panic が発生する:

```rust
// Line 184: ブロックスコープ変数
if let ScopeType::Block = scope_type {
    panic!("todo: block scoped variable is not implemented")
}

// Line 188: グローバル変数
if let ScopeType::Root = scope_type {
    panic!("todo: global variable is not implemented")
}
```

### 1.3 テスト状況

| テスト | 状況 | 問題 |
|--------|------|------|
| `scope_block_001` | ❌ 失敗 | "todo: block scoped variable is not implemented" |
| `scope_func_001` | ❌ 失敗 | trace の回数不一致（expect: [1,1,1,1], actual: 1回） |
| `scope_nested_func_001` | ❌ 失敗 | 詳細未確認 |
| `disabled_scope_block_var_001` | - | 無効化テスト |

**注意**: テストファイル自体がブロック内での `let:` を使用しているため、現状ではテストが実行できない。

---

## 2. 変更が必要なモジュール

### 2.1 semantic_analyzer/mod.rs

**現状の責務**:
- 構文木（Statement/Expression）を実行可能な構造（ExecStatement/ExecExpression）に変換
- 変数・関数の登録と識別子解決

**必要な変更**:
1. ブロックスコープ内での変数宣言を許可
2. スコープの親子関係を構築
3. 識別子解決時に親スコープを探索

**データ構造の変更案**:

```rust
// 現状: If/While は Vec<ExecStatement> を保持
pub enum ExecExpression {
    If(Box<ExecExpression>, Vec<ExecStatement>, Vec<ExecStatement>),
    While(Box<ExecExpression>, Vec<ExecStatement>),
    ...
}

// 変更案: Block 構造体を導入し、スコープ情報を保持
pub struct Block {
    pub scope: Scope,
    pub code: Vec<ExecStatement>,
}

pub enum ExecExpression {
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    ...
}
```

### 2.2 interpreter/mod.rs

**現状の責務**:
- `LocalEnvironment` で関数単位の変数を管理
- 変数名（String）を値（i64）にマッピング

**必要な変更**:
1. ブロックスコープに入るときに新しい変数マップをプッシュ
2. ブロックスコープを抜けるときにポップ
3. 変数アクセス時にスコープスタックを上から検索

**実行時スコープスタック案**:

```rust
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    root_scope: &'a Scope,
    // 変更: スコープのスタック（末尾が現在のスコープ）
    scope_stack: Vec<BTreeMap<String, i64>>,
}

impl LocalEnvironment<'_, '_> {
    fn enter_block(&mut self, scope: &Scope) {
        let mut new_vars = BTreeMap::new();
        for v in scope.variables.iter() {
            new_vars.insert(v.identifier.clone(), 0);
        }
        self.scope_stack.push(new_vars);
    }

    fn leave_block(&mut self) {
        self.scope_stack.pop();
    }

    fn get_variable(&mut self, name: &str) -> Option<&mut i64> {
        for scope in self.scope_stack.iter_mut().rev() {
            if let Some(val) = scope.get_mut(name) {
                return Some(val);
            }
        }
        None
    }
}
```

### 2.3 ExecExpression/ExecStatement の変更

**現状**:
- 変数は `Variable(String)` として文字列で保持
- 関数も `Function(String, ...)` として文字列で保持

**将来的な変更**:
- 識別子を解決済みの参照に変換（`Variable(Identifier)`）
- これにより実行時の名前検索が不要になる

---

## 3. 段階的実装の提案

大きな変更を避け、段階的に実装を進める。

### Phase 1: ブロックスコープ変数の最小実装

**目標**: ブロック内で `let:` を使用可能にし、シャドウイングをサポート

**スコープ**:
- if/while ブロック内での変数宣言を許可
- 同名変数のシャドウイング（ブロック内で外側と同じ名前の変数を定義可能）
- ブロックを抜けると変数は破棄

**除外**:
- グローバル変数
- static変数
- 識別子の事前解決（文字列のまま）

**変更対象**:
1. `semantic_analyzer/mod.rs`: ブロック内変数宣言の panic を除去、Block 構造体導入
2. `interpreter/mod.rs`: スコープスタックを導入

### Phase 2: 識別子の事前解決

**目標**: 変数名を意味解析時に解決し、実行時の名前検索をなくす

### Phase 3: グローバル変数

**目標**: 関数外での変数宣言をサポート

### Phase 4: static変数

**目標**: クロージャのような親関数スコープへのアクセス

---

## 4. 過去の実装（コミット 7a83612）との比較

過去の実装では以下のアプローチが取られていた:

1. **PendingScope / PendingBlock**: 2パス方式で識別子を解決
2. **Identifier 構造体**: `{ scope: usize, local: usize }` で識別子を一意に特定
3. **ScopeStackResolver**: 識別子解決時にスコープスタックを探索

この実装は複雑であったため revert されたが、設計思想は参考になる。

**簡略化の方針**:
- 識別子の事前解決は Phase 2 で行う
- Phase 1 では実行時に名前解決を行うシンプルな実装とする

---

## 5. 参考: 関連する言語仕様

```nospace
func: main() {
  let:a;
  a = 1;
  if:1{
    let:b;     # ブロック内での変数定義 #
    let:a;     # シャドウイング: 外側の a を隠す #
    a = 2;     # このブロック内の a に代入 #
    b = 3;
  };
  # ここで a == 1（外側の a は変更されていない）#
  # b は存在しない #
}
```
