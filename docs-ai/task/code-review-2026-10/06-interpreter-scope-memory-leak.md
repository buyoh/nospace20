# インタプリタ: 内部スコープ領域が解放されずメモリが際限なく増える

- 重要度: **high**
- 状態: 未着手

## 進捗

- [ ] 回収方式の決定
- [ ] 修正・長時間ループのテスト追加

## 問題点

関数呼び出し・ブロック進入ごとに `alloc_internal` で `MemoryBlock` が作られるが、`free_internal`（`src/interpreter/allocator.rs:207-215`）は `is_freed = true` にするだけで `blocks`（`BTreeMap`）から削除しない。`next_addr`（`:175-191`）は単調増加。

呼び出し元: `src/interpreter/exec.rs:89-96,237-300`、`src/interpreter/vm/scope.rs:31-40`

### 再現（関数 f を n 回呼ぶだけのループ）

| n | max RSS | 実行時間 |
|---|---|---|
| 1e5 | 26 MB | 0.5s |
| 1e6 | 222 MB | 6.1s |
| 3e6 | 658 MB | 19.4s |

1 呼び出しあたり約 220 バイトの線形増加。NospaceVM（WASM）も同じアロケータを使うため、長時間ループでブラウザタブが OOM になる。

## 違反している原則

- 資源管理・DoS 耐性
- ドキュメントコメント（LIFO 前提と記述）と実装の不一致

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `free_internal` で `blocks.remove(&addr)` し、末尾ブロックなら `next_addr` を巻き戻す（LIFO なのでスタックとして回収可能） | `allocator.rs`、`allocator_tests.rs`。解放済みローカル変数へのダングリング参照が「freed access」ではなく「invalid access」として報告されるようになる |
| B | 内部領域を汎用ヒープから切り離し、`Vec<i64>` のスタック（フレームポインタ方式）で管理する。WS コンパイラのローカルヒープと同じモデル | `allocator.rs`、`exec.rs`、`vm/scope.rs`、`vm/mod.rs` |

推奨: A（解放済み検出を残したいならブロックを tombstone 化して上限件数で回収）。
