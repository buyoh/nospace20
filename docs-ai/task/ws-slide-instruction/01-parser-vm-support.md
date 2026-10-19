# 01: Slide 命令のパーサ・VM 対応

`Instruction::Slide(WsNumber)` を追加し、whitespace20 で Slide を解釈・実行できるようにする。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

### 命令の意味

Whitespace v0.3: スタック先頭の値を保持したまま、その下にある `n` 個の値を取り除く。

```text
slide n:  [..., x_n, ..., x_1, top]  ->  [..., top]
```

| n | 本処理系 | wsc（2026-10-19 確認） |
|---|---|---|
| `0` | 何もしない（スタックが空なら `StackUnderflow`） | 同左 |
| `1 <= n < len` | 先頭を残し、その下の n 個を除去 | 同左 |
| `n >= len` | `StackUnderflow` | `Cannot discard more elements than items exist on stack` |
| `n < 0` | `StackUnderflow`（予約。将来の独自拡張で意味を割り当てる可能性あり） | 同上のエラー |

負の `n` は Copy（`|n|` として扱う）とは異なり **エラー** とする。

- Copy の `|n|` は既存実装（`unsigned_abs()`）の挙動を維持するための決定であり、互換性の制約がない新命令 Slide には当てはまらない
- エラーにしておけば、将来負のオペランドに独自の意味を割り当てても既存プログラムを壊さない（予約領域として最も自由度が高い）
- wsc と同じ結果になり、クロスバリデーションでも食い違わない

**要確認（ユーザー判断）**: 親ドキュメント #15 の注記「負のインデックスは独自仕様 API により予約されるケースがあるため、アクセス可能にしておく」を Slide にも適用して `|n|` とする案もある。実装着手前にどちらにするか確認すること。`|n|` 案にする場合は VM 実装の `n.0 < 0` 判定を削除し、spec の記載とテスト 5.3 を変更するだけでよい。

### 変更箇所

| ファイル | 変更内容 |
|---|---|
| `src/base/ws_types.rs` | `Instruction::Slide(WsNumber)`（コメント `// SP TB LF <n>`）、`encode()` で `[Space, Tab, Lf] + n.encode()`、`to_mnemonic()` で `"slide {n}"`（wsc のアセンブリ形式と同じ名前） |
| `src/whitespace/parser.rs` | `parse_stack_op` に `(Some(Tab), Some(Lf))` の分岐を追加し、`parse_number(chars, pos + 2)` で引数を読む |
| `src/whitespace/interpreter.rs` | `execute_instruction` に Slide を追加。`InstructionCounts` に `slide` フィールド、`record_instruction_count` に分岐を追加 |
| `examples/ws_profiler.rs` | `InstructionCountsYaml` に `slide` を追加し、`ic.slide` を詰める |
| `tools/profile-report.py` | 命令名一覧（`"push", "duplicate", "copy", ...`）に `"slide"` を追加 |
| `docs/spec-whitespace.md` | Copy タスクで新設する「処理系定義の挙動」節に、Slide の負の引数がエラーになることを追記（スタック操作表自体は記載済み） |
| `docs-ai/spec/compiler-rust-impl/whitespace.md` | 転記されている `Instruction` 列挙型・`encode` に `Slide` を追加 |
| `VERSION.md` | WIP に `whitespace20: Support the Slide instruction (Whitespace v0.3)` を追記 |

`Instruction` の `match` は網羅的に書かれているため、追加漏れはコンパイルエラーで検出できる。現状 `Instruction` 全体を網羅 match しているのは `ws_types.rs`（`encode` / `to_mnemonic`）と `interpreter.rs`（`record_instruction_count` / `execute_instruction`）のみ。コンパイラ・peephole は `matches!` / `if let` で個別命令を見ているだけで、Slide を生成しないため影響しない。

### VM 実装

```rust
Instruction::Slide(n) => {
    // 先頭を残して、その下の n 個を取り除く。負の n は予約のためエラー（docs/spec-whitespace.md 参照）
    if n.0 < 0 {
        return ExecuteResult::Error(RuntimeError::StackUnderflow);
    }
    let keep_len = match index_from_top(self.data_stack.len(), n.0.unsigned_abs()) {
        Some(i) => i,
        None => return ExecuteResult::Error(RuntimeError::StackUnderflow),
    };
    let top = self.data_stack[self.data_stack.len() - 1];
    self.data_stack.truncate(keep_len);
    self.data_stack.push(top);
    self.pc += 1;
}
```

- `index_from_top(len, n)` は「先頭から n 番目のインデックス = `len - 1 - n`」を返す。これは「除去後に残る下側の要素数」と一致するため、そのまま `truncate` の長さに使える
- `index_from_top` が `Some` を返す時点で `len >= n + 1 >= 1` が保証されるので、`top` の取得は panic しない
- `truncate` + `push` は O(1)（要素を移動しない）

## タスクステップ・チェックリスト

1. [ ] `ws_types.rs`: `Instruction::Slide`・`encode`・`to_mnemonic`
2. [ ] `parser.rs`: `parse_stack_op` に Slide 分岐
3. [ ] `interpreter.rs`: 実行・`InstructionCounts.slide`・`record_instruction_count`
4. [ ] `examples/ws_profiler.rs`・`tools/profile-report.py` に `slide` を追加
5. [ ] テスト追加
   1. [ ] `src/whitespace/parser_tests.rs`: `" \t\n \t\n"`（Slide(1)）が `Instruction::Slide(WsNumber(1))` にパースされる
   2. [ ] `src/base/ws_types_tests.rs`: `Slide(WsNumber(1)).encode()` が `[Space, Tab, Lf, Space, Tab, Lf]` になる（既存の `test_encode_push` の形式に合わせる）
   3. [ ] `src/whitespace/interpreter_tests.rs`: `[1,2,3]` に `Slide(1)` → `[1,3]`、`Slide(0)` → 不変、`Slide(3)` on `[1,2,3]` → `StackUnderflow`、空スタックで `Slide(0)` → `StackUnderflow`、`Slide(-1)` を十分な深さのスタックで実行しても `StackUnderflow`（負の n の予約）、`Slide(i64::MIN)` → `StackUnderflow`、`Slide(i64::MAX)` → `StackUnderflow`、プロファイリング有効時に `instruction_counts.slide` が加算される
   4. [ ] `resources/tests_ws/passes/stack/slide_001.wsa`（`push 1; push 2; push 3; slide 1; outnum; outnum; exit` → `31`。whitespace20 は末尾に `exit` がないと `ProgramCounterOutOfBounds` になるため、既存の `copy_001.wsa` と同様に `NNN` で終える）と `ws_io` の `.check.json`、`test-manifest.yaml` へ登録（wsc クロスバリデーションも通ることを確認）
   5. [ ] `resources/tests_ws/fails/runtime/slide_underflow_001.wsa`（`push 5; slide 3; exit`）と `ws_runtime_error` / `StackUnderflow` の `.check.json`、`test-manifest.yaml` へ登録
6. [ ] `docs/spec-whitespace.md`・`docs-ai/spec/compiler-rust-impl/whitespace.md`・`VERSION.md` 更新
7. [ ] `cargo test` 全体と `tools/wasm-test` が通ることを確認

### テスト補足

- 負の `n` は wsc でもエラーになるが、エラー種別の比較方法がクロスバリデーションと合うか未確認のため、ユニットテストのみで扱う
- `.wsa` の書き方は `resources/tests_ws/README.md`（`S`/`T`/`N` 記法）に従う。Slide 1 は `STN` + `STN`（命令 `S T N` + 数値 `+1` = `S T N`）
