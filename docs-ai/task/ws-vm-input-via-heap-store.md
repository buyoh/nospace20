# WhitespaceVM 入力命令のヒープ書き込みを heap_store に一元化

WhitespaceVM の InputChar / InputNumber がヒープへ直接 `heap.insert` している箇所を `heap_store` 経由に変更し、拡張 API フックとプロファイラ統計を Store 命令と同じ経路に揃える。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

親: [code-review-2026-10 #15](code-review-2026-10/15-ws-vm-instruction-inconsistency.md) の項目 3（解決策 C）

## 背景・動機

`src/whitespace/interpreter.rs` のヒープ書き込みは 2 経路に分かれている。

| 命令 | 書き込み方法 | 拡張 API フック（`--std-ext debug`） | プロファイラ統計 |
|---|---|---|---|
| Store | `heap_store(addr, val)` | 適用（-10 / -11 / -12） | `record_store` で記録 |
| InputChar / InputNumber | `self.heap.insert(addr, v)` | **適用されない** | **記録されない** |

その結果:

- `examples/ws_profiler.rs` の `heap_store_range` / `heap_store_count` / `heap_unique_addresses` に入力命令の書き込みが含まれず、`__geti` / `__getc` を使うプログラムのメモリ統計が不正確
- 負ヒープアドレスの拡張 API が「Store 命令でのみ有効」という暗黙の挙動になっており、ヒープ書き込みを一元管理するという `heap_store` の役割（DRY）が崩れている

## 設計・方針

### 方針: 入力命令の格納は Store と同じ「ヒープ書き込み」として扱う

入力命令の格納処理を `heap_store` に通す。これにより、ヒープへの書き込みは命令によらず次の扱いになる。

- 拡張 API: `--std-ext debug` 有効時、入力の格納先が -10 / -11 / -12 なら、入力値を引数とした `__trace` / `__assert` / `__assert_not` として動作する（ヒープには書き込まない）
- プロファイラ: 書き込みアドレスを `record_store` で記録する

拡張 API は「特定の負アドレスへの書き込み」を API 呼び出しとして解釈する方式（`docs-ai/architecture/whitespace-runtime.md`「推奨方式: ヒープ経由の間接呼び出し」）であり、書き込み元の命令で挙動を変える理由はない。
コンパイラが入力命令の格納先に使うのは `TEMP_PTR`（= 4）と変数アドレス（いずれも非負）のみのため、nospace からコンパイルしたプログラムの挙動は変わらない。影響を受けるのは手書きの Whitespace で `--std-ext debug` を有効にし、かつ負アドレス -10〜-12 に入力するプログラムだけである。

### 実装

```rust
Instruction::InputChar => {
    ...
    match self.read_char() {
        Ok(v) => {
            // バグ修正: 従来は self.heap.insert で直接書き込んでいたため、
            // --std-ext debug の負アドレスフックとプロファイラ統計を迂回していた。
            // 解決策: Store 命令と同じ heap_store を経由させる。
            if let Err(e) = self.heap_store(addr, v) {
                return ExecuteResult::Error(e);
            }
            self.pc += 1;
        }
        ...
    }
}
```

InputNumber も同様に変更する。

- 入力の読み取り後に `heap_store` がエラー（`AssertionFailed`）を返した場合、入力とアドレスの pop は消費済みのまま `StepResult::Error` を返す。Store 命令でも値・アドレスの pop 後にエラーになるのと同じ扱い。WhitespaceVM はエラー時に `completed` も `pc` も更新しないため、呼び出し側がエラー後に `step()` を再度呼ぶと同じ命令を再実行するが、これは Store を含む既存のエラー時挙動と同じであり、本タスクでは変更しない（CLI はエラー後に終了する。WASM の `WasmWhitespaceVM` はエラーを返すだけで再実行を禁止しておらず、エラー後に停止させるかは呼び出し側に任されている）
- `WouldBlock` 時はこれまでどおりアドレスをスタックに戻して待機し、`heap_store` は呼ばない（書き込み・統計の二重記録は起きない）

### プロファイラ統計の意味の変更

`HeapProfileStats.store_count` は現在「Store 命令の実行回数」と記載されている。変更後は入力命令による書き込みも数えるため、意味を「ヒープ書き込み回数（Store / InputChar / InputNumber）」に改める。`store_range` も同様に「書き込みアドレスの範囲」とする。
フィールド名（`store_count` / `heap_store_count`）は JSON 出力と `tools/profile-report.py` の互換性のため変更しない。Store 命令単独の回数は `instruction_counts.store` で取れる。

注意点:

- `__geti` / `__getc` を使う nospace プログラムでは、変更前後で `heap_store_count`・`heap_store_range`・`heap_unique_addresses` の値が変わる。変更前に取得したプロファイル JSON と `profile-report.py` で比較すると差分として現れるため、VERSION.md に明記する
- `instruction_counts` は命令の実行前に加算されるため、interactive stdin で `WouldBlock` から再試行すると `input_char` / `input_number` が二重に数えられる（既存の問題。ヒープ統計は `heap_store` 成功時のみ記録されるため影響しない）。本タスクの範囲外とし、「残課題」に記録する

| 対象 | 変更 |
|---|---|
| `src/whitespace/interpreter.rs` の `HeapProfileStats` | `store_count` / `store_range` / `record_store` のドキュメントコメントを「ヒープ書き込み（Store / 入力命令）」に修正 |
| `docs/profiler.md` | `execution.memory` の説明に「`heap_store_*` は Store 命令と入力命令による書き込みの合計」を追記 |
| `docs/spec-whitespace.md` | 拡張 API の「使用方法」「詳細仕様」の「Store 命令で…」を「ヒープへの書き込み（Store 命令、または InputChar / InputNumber の格納）で…」に修正 |

## タスクステップ・チェックリスト

1. [ ] `interpreter.rs`: InputChar / InputNumber の `self.heap.insert` を `heap_store` に置き換え（バグ原因・解決策のコメントを付与）
2. [ ] `HeapProfileStats` のドキュメントコメント修正
3. [ ] ユニットテスト追加（`src/whitespace/interpreter_tests.rs`）
   1. [ ] プロファイリング有効で InputChar / InputNumber を実行すると `heap.store_count`・`store_range`・`unique_address_count` に反映される
   2. [ ] `with_debug_ext(true)` で -10 に InputNumber すると `traced` に入力値が記録され、ヒープ（`vm.heap()`）には書き込まれない
   3. [ ] `with_debug_ext(true)` で -11 に InputChar し、EOF（入力値 0）だと `AssertionFailed(0)` になる
   4. [ ] `with_debug_ext(false)` では -10 への入力が通常のヒープ書き込みになる（従来挙動の維持）
   5. [ ] interactive stdin で `WouldBlock` → `provide_stdin` → 再実行した場合に、`store_count` が 1 回だけ加算される
4. [ ] `docs/spec-whitespace.md`・`docs/profiler.md` 更新
5. [ ] `VERSION.md` の WIP に追記（例: `whitespace20: InputChar / InputNumber now write the heap through the same path as Store (debug extension API addresses apply to input, and profiler heap statistics now include input writes)`）
6. [ ] `cargo test` 全体が通ることを確認
7. [ ] 親ドキュメント（#15）の進捗を更新

## 残課題

- interactive stdin の `WouldBlock` 再試行時に `instruction_counts.input_char` / `input_number` が二重加算される（`record_instruction_count` が実行前に呼ばれるため）。必要になったら別タスクとして起票する
