# while ループ本体のスタックリーク修正

## 概要

`test_example_qsort_ws_self` が失敗している原因は、`generate_while_expression` がループ本体に
`generate_block` を使用しており、`generate_block` が各呼び出し末尾で生成する `push 0`（ブロックの
式値）がイテレーションごとにスタックに蓄積し、関数の `generate_local_deallocate` でスタック不整合を
引き起こしていることであった。

## 根本原因の詳細

### Bug C: while ループ本体のブロック値がスタックに蓄積

#### `generate_block` の動作

```rust
pub fn generate_block(ctx, block) -> WsProgram {
    let mut prog = WsProgram::new();
    for stmt in &block.statements {
        prog.append(generate_statement(ctx, stmt)?);
    }
    prog.push(Instruction::Push(WsNumber(0)));  // ← ブロックの式値
    Ok(prog)
}
```

各 `generate_block` 呼び出しは最後に `push 0` を生成し、呼び出し元がこの値を消費する
ことを期待している。

#### `generate_while_expression` の動作

```rust
fn generate_while_expression(ctx, cond, body) {
    // ...
    label_start:
      condition evaluation
      jz label_end
      generate_block(ctx, body)     // ← push 0 が生成される
      jmp label_start
    label_end:
      push 0                        // while 式の値
}
```

ループ本体の `generate_block` で生成される `push 0` は **消費されない**。
各イテレーションでスタックに 1 つずつ値が蓄積される。

#### N回のイテレーション後のスタック状態

```
entry: stack = [..., old_LHB]
iteration 1 body: stack = [..., old_LHB, 0]
iteration 2 body: stack = [..., old_LHB, 0, 0]
...
iteration N body: stack = [..., old_LHB, 0, 0, ..., 0]  (N 個の余分な 0)
```

### deallocate での破壊

`generate_local_deallocate` はスタックトップが `old_LHB` であることを前提としている:

```
push 3; push 2; retrieve; store   ; heap[3] = heap[2]  (LHE = LHB)
push 2; swap; store               ; heap[2] = スタックトップ ← old_LHB であるべき
```

しかし、while ループ後のスタックトップは余分な `0` であるため:

```
push 2; swap; store → heap[2] = 0  ← BUG! (old_LHB ではなく 0 を復元)
```

### qsort テストでの影響

1. `qsort` 内の while ループで `0` がスタックに蓄積
2. `qsort` のデフォルト return（関数末尾）で deallocate が `heap[2] = 0` を書き込み
   （正しくは main の `LOCAL_HEAP_BEGIN` = 8）
3. main に戻ると `LOCAL_HEAP_BEGIN = 0` のため:
   - `n` の読み込み: `heap[0 + 20] = heap[20]` → 未初期化 → 0
   - while 条件 `i < n` が `0 < 0` → 偽
   - ループ未実行 → **出力が空**

### 他テストが通る理由

- **fibonacci**: while ループを持つが、return 後に main がローカル変数にアクセスしない。
  main の処理は `__puti(fibo(__geti()))` であり、fibo が heap[2] を破壊しても
  main は fibo の戻り値をそのまま出力するだけで、ローカル変数参照が不要。
  main の default return で heap[2] は正しく復元される。
- **while ループなしの関数**: スタック汚染が発生しないため問題なし。
- **再帰なしの while ループ**: main 内の while であれば、main 自身の return 時に
  汚染されるが、プログラム終了（exit）で影響が出ない。

### `generate_block` を呼ぶ全箇所の確認

| 呼び出し元 | ブロック値の消費 | 問題 |
|---|---|---|
| `generate_if_expression` (then/else) | if 式の値として消費される | なし |
| `generate_while_expression` (body) | **消費されない** | **BUG** |
| `ExecExpression::Block` | 式の値として消費される | なし |

---

## 修正設計

### Fix C: while ループ本体のブロック値を Discard

**対象ファイル**: `src/compiler_ws/expression.rs` の `generate_while_expression`

**変更内容**: while ループ本体の `generate_block` 直後に `Instruction::Discard` を追加し、
ブロック値をスタックから除去する。

```rust
// 修正前
fn generate_while_expression(ctx, cond, body) {
    // ...
    prog.append(super::statement::generate_block(ctx, body)?);
    prog.push(Instruction::Jump(loop_start));
    // ...
}

// 修正後
fn generate_while_expression(ctx, cond, body) {
    // ...
    prog.append(super::statement::generate_block(ctx, body)?);
    prog.push(Instruction::Discard);  // ← ブロック値をクリーンアップ
    prog.push(Instruction::Jump(loop_start));
    // ...
}
```

### スタックトレース検証

**修正後の N 回イテレーション:**

```
entry: stack = [..., old_LHB]
iteration 1: body push 0 → discard → stack = [..., old_LHB]
iteration 2: body push 0 → discard → stack = [..., old_LHB]
...
iteration N: body push 0 → discard → stack = [..., old_LHB]
loop exit: push 0 (while 式値) → stack = [..., old_LHB, 0]
```

**default return の場合:**

```
stack: [..., old_LHB]   (while 式値は ExecStatement::Expression で discard 済み)
deallocate: heap[2] = old_LHB ✓
push 0; ret → 正常
```

**explicit return の場合:**

```
stack: [..., old_LHB]
push return_value: [..., old_LHB, return_value]
swap (Fix B): [..., return_value, old_LHB]
deallocate: heap[2] = old_LHB ✓
ret → return_value を返す ✓
```

### 既存テストへの影響

- `generate_while_expression` のブロック値は従来どこにも使用されていなかったため、
  Discard の追加は論理的に安全
- while 式の値自体（ループ後の `push 0`）は引き続き生成されるため、while 式を
  式として使用するコード（`let: x = while: ... {};`）への影響はない

---

## ステータス

- [x] 根本原因の特定（while ループ本体のブロック値蓄積）
- [x] 影響範囲の分析（`generate_block` 呼び出し箇所の全数確認）
- [x] 修正設計（Fix C: Discard 追加）
- [ ] Fix C の実装
- [ ] テスト通過確認（`test_example_qsort_ws_self` および全 ws_self テスト）

## 関連ドキュメント

- [remaining-ws-self-failures.md](remaining-ws-self-failures.md) - Fix A/B の実装記録
- [qsort-ws-self-failure.md](qsort-ws-self-failure.md) - 元の調査起票
