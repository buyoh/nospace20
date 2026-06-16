# Bug D: 原因分析

## 問題

`test_example_qsort_ws_self` の出力が `"0 0 0 1 1 4 7 "` となり、期待値 `"1 1 2 3 4 5 9 "` と不一致。

## 根本原因

**内部ブロックスコープの変数が関数スコープの変数とヒープメモリアドレスを共有（衝突）する。**

### インタプリタとコンパイラの差異

インタプリタは `scope_stack: Vec<Vec<i64>>` を使い、各ブロックスコープが独立した `Vec<i64>` を持つ。
`IdentifierRef.scope_depth` でスタック内のスコープを特定し、`local_index` でアクセスする。
スコープ間のメモリ衝突は発生しない。

```
// interpreter/exec.rs
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
    self.scope_stack[scope_idx][id.local_index]
}
```

一方、Whitespace コンパイラは全ローカル変数をヒープの `heap[LHB + offset]` で一元管理し、
`CodeGenContext::get_var_info` は `scope_depth` を無視して `local_index` をそのまま `offset` として使う。

```
// compiler_ws/context.rs
pub fn get_var_info(&self, var_ref: &IdentifierRef) -> VarInfo {
    VarInfo {
        scope: VarScope::Local,
        offset: var_ref.local_index as i64,  // scope_depth を無視
    }
}
```

### qsort main() での具体例

```nospace
func: main() {
  let: arr[20];      // slot 0-19 → heap[LHB+0] ~ heap[LHB+19]
  let: n;            // slot 20   → heap[LHB+20]
  ...
  {
    let: i(0);       // slot 0    → heap[LHB+0] ← arr[0] と衝突!
    while: i < n {
      arr[i] = __geti();
      i += 1;
    };
  };
  let: i(0);         // slot 21   → heap[LHB+21] (関数スコープなので問題なし)
  ...
}
```

内部ブロックのスコープは独立した `variable_count` を持ち、`slot_index` が 0 から始まる。
コンパイラはこれを `heap[LHB+0]` にマッピングするため、`arr[0]` と衝突する。

### 実行トレースによる出力再現

stdin: `7\n3\n1\n4\n1\n5\n9\n2\n`

1. `n = __geti()` → `heap[LHB+20] = 7`
2. 内部ブロック: `i = 0` → `heap[LHB+0] = 0` (arr[0] を破壊)
3. ループ:
   - `i=0`: `arr[0] = __geti()` → `heap[LHB+0] = 3` → i も 3 に変化
   - `i += 1` → `heap[LHB+0] = 4` → arr[0]=4, i=4
   - `i=4`: `arr[4] = __geti()` → `heap[LHB+4] = 1`
   - `i += 1` → `heap[LHB+0] = 5`
   - `i=5`: `arr[5] = __geti()` → `heap[LHB+5] = 4` (stdin "5" ではない。stdinの4番目の値)
   - `i += 1` → `heap[LHB+0] = 6`
   - `i=6`: `arr[6] = __geti()` → `heap[LHB+6] = 1`
   - `i += 1` → `heap[LHB+0] = 7`, `i=7 >= n=7` → ループ終了
4. 配列の状態: `[7, 0, 0, 0, 1, 4, 1]` (arr[1,2,3] は未初期化)
5. qsort → `[0, 0, 0, 1, 1, 4, 7]`
6. 出力: `"0 0 0 1 1 4 7 "` ← 実際の出力と一致

### 他テストが成功する理由

115 テスト中 114 テストが成功しているのは、
内部ブロックスコープで `let:` 宣言する（かつ親スコープ変数と衝突する）ケースが qsort テスト特有であるため。
