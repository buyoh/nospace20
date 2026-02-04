# Phase 2: 識別子の事前解決 - 完了レポート

## 実装日

2026-02-05

## 概要

意味解析時に変数名を解決し、実行時の文字列検索を排除することで、パフォーマンスを向上させる Phase 2 を完了しました。

## 実装内容

### 1. IdentifierRef 構造体の追加

```rust
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRef {
    pub scope_depth: usize,  // スコープの深さ
    pub local_index: usize,  // スコープ内でのインデックス
}
```

### 2. Scope 構造の拡張

- `variable_indices: BTreeMap<String, usize>` - 変数名からインデックスへのマップ
- `variable_count: usize` - 変数の総数

### 3. ExecExpression の変更

- `Variable(String)` → `Variable(IdentifierRef)`
- 関数は組み込み関数のみのため `Function(String, ...)` のまま保持

### 4. ScopeResolver の実装

```rust
struct ScopeResolver<'a> {
    scope_stack: Vec<&'a BTreeMap<String, usize>>,
}
```

- 変数名を IdentifierRef に解決する機能
- 親スコープの変数も参照可能

### 5. 2パス解析の実装

- **パス1**: 変数宣言の収集（ホイスティング対応）
- **パス2**: 識別子解決と ExecExpression への変換

### 6. インタプリタの Vec<i64> 化

- `scope_stack: Vec<BTreeMap<String, i64>>` → `scope_stack: Vec<Vec<i64>>`
- `get_variable(id: &IdentifierRef) -> i64` - O(1) アクセス
- `set_variable(id: &IdentifierRef, value: i64)` - O(1) 更新

## パフォーマンス改善

| 操作 | Phase 1 (文字列) | Phase 2 (インデックス) |
|------|----------------|---------------------|
| 変数読み取り | O(depth × log n) | O(1) |
| 変数書き込み | O(depth × log n) | O(1) |
| メモリ | 変数名を複製 | インデックスのみ |

## 技術的課題と解決策

### 課題1: ホイスティングへの対応

**問題**: 変数宣言より前に変数を使用できる
```nospace
a = 5;    # 使用 (先)
let: a;   # 定義 (後)
```

**解決**: 2パス解析
- パス1で全変数を収集
- パス2で識別子を解決

### 課題2: 親スコープの変数参照

**問題**: while/if のブロック内で親スコープの変数が参照できない

**解決**: `analyze_internal_with_parent` の実装
- 親の `ScopeResolver` を継承
- スコープスタックに親のマップを含める

### 課題3: 関数引数の扱い

**問題**: 関数本体を解析する前に引数を登録する必要がある

**解決**: `initial_vars` パラメータ
- 関数引数を初期変数として渡す
- パス1の前に登録

## テスト結果

全テスト通過:
- ユニットテスト: 69 passed
- 統合テスト: 64 passed
- **合計**: 133 tests passed ✅

## 未実装・制限事項

1. **関数は組み込み関数のみ**
   - ユーザー定義関数の呼び出しは Phase 3 で対応予定
   - 現状は `ExecExpression::Function(String, ...)` のまま

2. **グローバル変数は未実装**
   - Phase 3 以降で対応予定

## コミット

- コミットハッシュ: fe0b77f
- メッセージ: "feat: Phase 2 - 識別子の事前解決を実装"

## 次のステップ

Phase 3 以降で以下を実装予定:
- グローバル変数のサポート
- ユーザー定義関数の IdentifierRef 化
- static 変数
- ネスト関数の可視性ルール

## 参考ドキュメント

- [phase2-identifier-resolution.md](../task/scope/phase2-identifier-resolution.md) - 設計ドキュメント
- [overview.md](../task/scope/overview.md) - スコープ機能の全体像
