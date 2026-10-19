# WhitespaceVM Copy 命令のインデックス変換修正

WhitespaceVM の `Copy(n)` で `n` を `usize` に変換する箇所が wasm32 で切り詰められる問題を修正し、負の `n` の扱いを本処理系の挙動として明文化する。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

親: [code-review-2026-10 #15](code-review-2026-10/15-ws-vm-instruction-inconsistency.md) の項目 1

## 背景・動機

`src/whitespace/interpreter.rs` の `Instruction::Copy(n)`（現在 597-609 行付近）は以下の実装になっている。

```rust
let idx = match self
    .data_stack
    .len()
    .checked_sub(1 + n.0.unsigned_abs() as usize)
```

- `n.0.unsigned_abs() as usize` は wasm32（`usize` = 32 bit）で上位ビットが切り捨てられる。`Copy(2^32)` が `Copy(0)` として成功し、WASM 版と CLI 版で結果が食い違う
- `1 + ...` は無検査の加算。wasm32 では `Copy(2^32 - 1)` で `1 + u32::MAX` が溢れ、release ビルドでは 0 に折り返して `checked_sub(0)` が `Some(len)` となり `data_stack[len]` の範囲外アクセスで trap する（debug ビルドでは加算オーバーフローで panic）。64 bit 環境では `unsigned_abs()` の最大値が `2^63` のため溢れない
- 負の `n` は `unsigned_abs()` によって `|n|` として扱われる。Whitespace 仕様では未定義で、wsc は `Tried to copy from outside the stack` エラーになる（2026-10-19 に `tools/wsc-install/bin/wsc` で確認）

## 設計・方針

### 負の n の扱い（方針決定済み）

親ドキュメントの決定（案 A を取り下げ）に従い、**負の `n` をエラーにしない**。
負のインデックスは将来の独自拡張 API に予約する可能性があるため、本タスクでは現状の挙動（`|n|` 番目をコピー）を維持し、それを「本処理系の挙動（将来変更の可能性あり）」として `docs/spec-whitespace.md` に記載する。

| n | 本処理系 | wsc |
|---|---|---|
| `0 <= n < len` | n 番目（0 = 先頭）をコピー | 同左 |
| `n >= len` | `StackUnderflow` | エラー |
| `n < 0` | `\|n\|` 番目をコピー（予約・将来変更の可能性あり） | エラー |

wsc と挙動が異なるため、負の `n` のテストは wsc クロスバリデーションの対象になる `resources/tests_ws/` には追加せず、ユニットテストで扱う。

### インデックス変換の修正

スタック先頭からの深さ `depth: u64` を実インデックスに変換する処理を、プラットフォーム非依存の小さな関数に切り出す。Slide 命令（[ws-slide-instruction](ws-slide-instruction/README.md)）でも同じ変換が必要になるため共用する。

```rust
/// スタック先頭から depth 番目（0 = 先頭）の要素のインデックスを返す。
/// 範囲外（usize に収まらない場合を含む）なら None
fn index_from_top(stack_len: usize, depth: u64) -> Option<usize> {
    let depth = usize::try_from(depth).ok()?;
    stack_len.checked_sub(depth.checked_add(1)?)
}
```

`Copy(n)` 側:

```rust
Instruction::Copy(n) => {
    // バグ修正: 従来は `n.0.unsigned_abs() as usize` で変換しており、wasm32 では
    // 上位ビットが切り捨てられて Copy(2^32) が Copy(0) として成功していた。
    // 解決策: usize::try_from と checked_add で変換し、収まらない場合は StackUnderflow とする。
    // 負の n は |n| として扱う（本処理系の挙動。docs/spec-whitespace.md 参照）
    let idx = match index_from_top(self.data_stack.len(), n.0.unsigned_abs()) {
        Some(i) => i,
        None => return ExecuteResult::Error(RuntimeError::StackUnderflow),
    };
    ...
}
```

- `index_from_top` は `WhitespaceVM` の関連関数ではなく `interpreter.rs` 内のモジュールプライベート関数とする（状態を持たない純粋関数のため）
- エラー種別は既存の `RuntimeError::StackUnderflow` を流用する（新しいバリアントは追加しない）

## タスクステップ・チェックリスト

1. [ ] `index_from_top` を `src/whitespace/interpreter.rs` に追加し、`Copy` をそれ経由に変更（バグ原因・解決策のコメントを付与）
2. [ ] ユニットテスト追加（`src/whitespace/interpreter_tests.rs`）
   1. [ ] `index_from_top(1, 1u64 << 32)` が `None`（wasm32 で切り詰められていた値。64 bit でも範囲外として `None`）
   2. [ ] `index_from_top(len, u64::MAX)` が `None`（`checked_add` の溢れ。64 bit で `usize::MAX + 1`）
   3. [ ] `index_from_top(1, u32::MAX as u64)` が `None`、`Copy(WsNumber(u32::MAX as i64))` が `StackUnderflow`（wasm32 で加算が溢れて trap していた値）
   4. [ ] `Copy(WsNumber(i64::MIN))` / `Copy(WsNumber(i64::MAX))` が `StackUnderflow`
   5. [ ] `Copy(WsNumber(-1))` が `Copy(1)` と同じ結果（負の n の現行挙動の固定）
3. [ ] `docs/spec-whitespace.md` に「処理系定義の挙動」節を新設し（「拡張仕様」節は `--std-ext` 指定時のみ有効な機能の節なので使わない）、「Copy の負の引数は `|n|` として扱う（Whitespace 仕様では未定義。将来の独自拡張のため予約しており、挙動は変更されうる）」を追記
4. [ ] `VERSION.md` の WIP に追記（例: `whitespace20: Copy with an out-of-range index is now a stack underflow error on every platform (was truncated or trapped on WASM)`）
5. [ ] `cargo test` と `tools/wasm-test` の既存テストが通ることを確認
6. [ ] 親ドキュメント（#15）の進捗を更新

### 補足

- 現在 CI で wasm32 向けのユニットテストは実行していないため、切り詰めの回帰は `index_from_top` が `usize::try_from` を使っていることで担保する。テスト 2.1・2.3 は wasm32 で実行された場合に切り詰めを検出できる形にしてある
- コンパイラ（`src/compiler_ws/`）は `alloc_runtime/` で `Copy` に 1〜3 の定数しか生成しないため、コンパイル結果への影響はない
