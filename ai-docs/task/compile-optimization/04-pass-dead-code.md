# 未使用関数・変数の削除

## 概要

`main` 関数から到達不可能な関数を特定し、コンパイル対象から除外する。未使用変数の削除も将来的に対応するが、初期実装では関数のみ。

## 未使用関数の削除

### 到達可能性解析

`main` 関数をルートとして、呼び出しグラフを構築し、到達可能な関数を特定する。

```
アルゴリズム:
1. reachable = { main }
2. worklist = [main]
3. while worklist is not empty:
     f = worklist.pop()
     for each function g called by f:
       if g not in reachable:
         reachable.add(g)
         worklist.push(g)
4. unreachable = all_functions - reachable
```

### 呼び出しの検出

`ExecExpression` を再帰的に走査し、`UserFunction(IdentifierRef, ...)` を収集する。

```rust
fn collect_called_functions(expr: &ExecExpression) -> Vec<usize> {
    match expr {
        ExecExpression::UserFunction(func_ref, args) => {
            let mut result = vec![func_ref.local_index];
            for arg in args {
                result.extend(collect_called_functions(&arg.expression));
            }
            result
        }
        ExecExpression::If(cond, then_block, else_block) => {
            // cond, then_block, else_block を再帰
            ...
        }
        // 他のバリアントも再帰
        ...
    }
}
```

走査対象:

- ルートスコープの `static_init_statements`
- ルートスコープの `root_statements`
- 各関数の `block` 内の文・式

### 削除方法

`Scope.functions` は `Vec<Function>` でインデックスベースのアクセスが行われるため、要素を直接削除するとインデックスがずれる。

**方針: 削除ではなく、空の関数に置換**

```rust
// 未使用関数を空のダミー関数に置換
for idx in unreachable_indices {
    scope.functions[idx] = Function::dummy();
}
```

`Function::dummy()` は空のブロックと `return_type = Void` を持つ最小限の関数。

### Compiler WS への影響

未使用関数のコード生成がスキップされるため、生成コードサイズが削減される。コンパイラ側で `Function::is_dummy()` をチェックし、ダミー関数のコード生成をスキップする。

### main が存在しない場合

`main` 関数が存在しない場合、全関数が到達不可能となる。この場合は最適化をスキップし、既存の動作を維持する。

## 未使用変数の削除（将来）

### 課題

- `variable_count` やスロットインデックスの再計算が必要
- `IdentifierRef.local_index` が無効になるリスク
- グローバル変数のメモリレイアウトに影響

### 段階的アプローチ

1. **Phase 1**: 未使用変数の検出・警告のみ（削除しない）
2. **Phase 2**: 未使用変数のスロットを 0 初期化せず省略（メモリレイアウトは変更しない）
3. **Phase 3**: 変数スロットの再配置（インデックス再計算が必要）

初期実装では Phase 1 のみを計画。

## 実装手順

1. `optimizer/dead_code.rs` を作成
2. 呼び出しグラフの構築（`Scope` 全体を走査）
3. 到達可能性解析（BFS/DFS）
4. 未到達関数をダミーに置換
5. `Function` に `is_dummy()` メソッドを追加
6. Compiler WS でダミー関数のコード生成をスキップ
7. テスト: 未使用関数を含むテストケースで動作確認

## 注意事項

- グローバル変数の初期化式から呼ばれる関数も到達可能とする
- static 変数の初期化式も走査対象
- 入れ子関数（関数内関数）は親関数が到達可能な場合のみ到達可能（ただし現在の実装ではすべてグローバルインデックスを持つ）
