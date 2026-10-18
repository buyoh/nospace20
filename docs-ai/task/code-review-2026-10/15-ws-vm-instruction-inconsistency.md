# WhitespaceVM の命令実装の細かな不整合

- 重要度: **low**
- 状態: 未着手

## 進捗

- [ ] Copy の負インデックス
- [ ] Slide 命令対応
- [ ] 入力命令の heap_store 経由化

## 問題点

`src/whitespace/interpreter.rs`、`src/whitespace/parser.rs`

1. **Copy(n)**（`interpreter.rs:597-609`）: `n.0.unsigned_abs()` のため負のインデックスが正として受理される（Whitespace では負の Copy はエラー）。`as usize` は wasm32 で切り詰められ、`Copy(2^32)` が `Copy(0)` として成功する。
2. **Slide 命令（`S T LF n`）未対応**: `parse_stack_op`（`parser.rs:60-85`）が `InvalidCommand` を返す。whitespace20 は汎用 Whitespace インタプリタとして公開されているが、WS 0.3 の標準プログラムを実行できない。
3. **InputChar / InputNumber**（`interpreter.rs:776,797`）が `heap_store` を経由せず `heap.insert` を直接呼ぶ。`--std-ext debug` の負アドレスフック（-10/-11/-12）を迂回し、プロファイラ統計（`unique_address_count` 等）にも記録されない → `examples/ws_profiler.rs` の出力が不正確。

## 違反している原則

- Whitespace 仕様準拠、ストア処理の一元化（DRY）

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | ~~Copy の負の n をエラーにし、`usize::try_from` で変換~~ | `interpreter.rs` |
| B | Slide を `base::ws_types::Instruction` に追加 | `ws_types.rs`、`parser.rs`、`interpreter.rs`、プロファイラの `InstructionCounts` と YAML 構造体 |
| C | 入力命令の書き込みを `heap_store` に通す | `interpreter.rs` のみ |
