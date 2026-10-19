# 意味解析: 配列・struct サイズ計算の usize 演算が未チェック、上限検証なし

- 重要度: **medium**
- 状態: 設計完了（未実装）。設計: [array-size-overflow](../array-size-overflow/README.md)。案 A のみ実施、案 B は不採用

## 進捗

- [ ] checked 演算化
- [ ] サイズ上限の仕様化
- [ ] fails テスト追加

## 問題点

- `src/semantic_analyzer/mod.rs:212`（`slot_index += var.array_size.unwrap_or(1)`）
- `src/semantic_analyzer/mod.rs:940`（`offset += size`）
- `src/semantic_analyzer/mod.rs:1016`（`inner_size * size`）
- `src/semantic_analyzer/mod.rs:694`（`n as usize`）

### 再現

- `let: a[9223372036854775807], b[9223372036854775807], c[9223372036854775807];` → debug: `mod.rs:212:9: attempt to add with overflow`
- `struct: S (d@int[4611686018427387904]); struct: T (x@S[8]);` → `mod.rs:1016 attempt to multiply with overflow`
- release（overflow-checks 無効）では wrap し、変数スロット配置が壊れたまま WS コードが出力される
- 単一の巨大配列 `let: a[9223372036854775807];` は意味解析を通過し、実行時にインタプリタ `allocator.rs:179` の overflow / `capacity overflow` で panic

## 違反している原則

- 整数オーバーフロー対策（既存指摘はトークナイザの数値リテラルのみ）
- フェイルセーフ設計（release で誤ったコードを黙って生成）

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `checked_add` / `checked_mul` に置換し、失敗時 `CodeParseError` | `semantic_analyzer/mod.rs` のみ |
| B | 配列サイズ・struct 合計サイズに上限（例: `MAX_ARRAY_SIZE_SLOTS`）を設け、ヒープ空間の想定と合わせる | `semantic_analyzer/mod.rs`、`docs/spec.md` 追記、fails テスト追加 |

推奨: A + B。
