# WhitespaceVM の命令実装の細かな不整合

- 重要度: **low**
- 状態: 設計済み（項目ごとに別タスク化）

## 進捗

- [ ] Copy のインデックス変換（wasm32 切り詰め。負の n は維持）→ [ws-vm-copy-index-conversion.md](../ws-vm-copy-index-conversion.md)
- [ ] Slide 命令対応 → [ws-slide-instruction/](../ws-slide-instruction/README.md)
- [ ] 入力命令の heap_store 経由化 → [ws-vm-input-via-heap-store.md](../ws-vm-input-via-heap-store.md)

## 問題点

`src/whitespace/interpreter.rs`、`src/whitespace/parser.rs`

1. **Copy(n)**（`interpreter.rs:597-609`）: `n.0.unsigned_abs()` のため負のインデックスが正として受理される（Whitespace では負の Copy はエラー）。`as usize` は wasm32 で切り詰められ、`Copy(2^32)` が `Copy(0)` として成功する。
2. **Slide 命令（`S T LF n`）未対応**: `parse_stack_op`（`parser.rs:60-85`）が `InvalidCommand` を返す。whitespace20 は汎用 Whitespace インタプリタとして公開されているが、WS 0.3 の標準プログラムを実行できない。
3. **InputChar / InputNumber**（`interpreter.rs:776,797`）が `heap_store` を経由せず `heap.insert` を直接呼ぶ。`--std-ext debug` の負アドレスフック（-10/-11/-12）を迂回し、プロファイラ統計（`unique_address_count` 等）にも記録されない → `examples/ws_profiler.rs` の出力が不正確。

### NOTE

Slide は Whitespace V0.3 で導入された命令。

導入する場合、以下の手続きが必要

- docs/spec-whitespace.md に V0.3 の命令を追記する（記載済み）
- これらの命令が拡張命令を阻害しないか確認
- コンパイラがこれらの命令を使うことでより効率的なコード生成が可能になるか検討

## 違反している原則

- Whitespace 仕様準拠、ストア処理の一元化（DRY）

## 解決策

A: 負のインデックスは独自仕様APIにより予約されるケースがあるため、アクセス可能にしておく。Whitespace では未定義動作のため、エラーにしない方針でも良い。

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | ~~Copy の負の n をエラーにし、`usize::try_from` で変換~~ | `interpreter.rs` |
| A' | `usize::try_from` + `checked_add` による変換のみ採用し、負の n は `\|n\|` のまま維持（予約扱いとして spec に明記） | `interpreter.rs`、`docs/spec-whitespace.md` |
| B | Slide を `base::ws_types::Instruction` に追加 | `ws_types.rs`、`parser.rs`、`interpreter.rs`、プロファイラの `InstructionCounts` と YAML 構造体 |
| C | 入力命令の書き込みを `heap_store` に通す | `interpreter.rs` のみ |
