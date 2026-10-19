# 配列・struct サイズ計算のオーバーフロー対策

[コードレビュー 2026-10 #08](../code-review-2026-10/08-array-size-overflow.md) の対応。
変数・struct のサイズ（スロット数）の計算を checked 演算にし、オーバーフローしたらコンパイルエラーを返す（案 A）。サイズの上限（案 B）は設けない。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 背景・動機

[08-array-size-overflow.md](../code-review-2026-10/08-array-size-overflow.md) の指摘を、現在のソース（コミット 84616dd 時点）で再確認した結果:

| # | 位置 | 内容 | 状況 |
|---|---|---|---|
| 1 | `src/semantic_analyzer/mod.rs:212` | スコープ内の変数のスロット番号の累積 `slot_index += array_size` | 未修正。debug で panic、release で wrap |
| 2 | `src/semantic_analyzer/scope.rs:710` | `ScopeBuilder::build()` で同じ累積をもう一度行っている（指摘に無い） | 未修正。#1 と同じ計算の重複 |
| 3 | `src/semantic_analyzer/mod.rs:940` | struct のフィールドオフセットの累積 `offset += size` | 未修正 |
| 4 | `src/semantic_analyzer/mod.rs:1016` | 配列型のサイズ `inner_size * size` | 未修正 |
| 5 | `src/semantic_analyzer/mod.rs:694` | `n as usize`（`n: i64`、正であることはパーサが保証） | 64bit では安全。wasm32 では上位ビットが切り捨てられる |

指摘に無いが、同じ値が流れ込む先にも未チェックの変換・演算がある（詳細は [01](01-checked-size-arithmetic.md)）:

- パーサの型注釈・struct フィールドの配列サイズ `*n as usize`（wasm32 で切り捨て）
- compiler_ws の `usize as i64` 変換と、関数フレーム・グローバル領域のサイズの合計
  - 意味解析で `usize` に収まっても、`i64` に収まるとは限らない（64bit で `usize::MAX` は `i64::MAX` の約 2 倍）
  - 関数フレームのサイズは、ネストしたブロックの変数の合計。スコープごとの検査では合計のオーバーフローを防げない

## 設計・方針

| 項目 | 方針 |
|---|---|
| 採用する案 | **A のみ**。サイズ計算を checked 演算にし、オーバーフローは意味解析エラー / コンパイルエラーにする |
| 案 B（サイズ上限） | **実施しない**。理由と、仮に実施する場合の検査箇所は [02](02-size-limit-option-b.md) に記録する |
| 検査の範囲 | 「値が型に収まること」だけを保証する。意味解析は `usize`、compiler_ws は WS のアドレス（`i64`）に収まることを検査する |
| 巨大だが表現できるサイズ | 受理する（例: `let: a[9223372036854775807];` 単独）。WhitespaceVM のヒープは `HashMap` なので確保は起きない。実際に使ったメモリ量は [ws-vm-run-mode-support/01](../ws-vm-run-mode-support/01-memory-limit.md) の `max_memory_cells` が実行時に制限する |
| 直接実行インタプリタ | 対象外。`alloc_internal_uninit(variable_count)` が巨大な `Vec` を確保して panic する問題（`allocator.rs:179`）は、[retire-direct-interpreter](../retire-direct-interpreter/README.md) でインタプリタごと廃止されるため対応しない |
| spec.md | 「配列・struct の合計サイズがアドレス空間（64bit 符号付き整数）を超える場合はコンパイルエラー」の 1 文を追記する。上限値は書かない |

## タスクステップ・チェックリスト

1. [ ] 01: サイズ計算の checked 化（[01-checked-size-arithmetic.md](01-checked-size-arithmetic.md)）
   1. [ ] パーサの配列サイズを `usize::try_from` に
   2. [ ] 意味解析のスロット累積・struct オフセット・配列サイズを checked に
   3. [ ] compiler_ws のフレーム・グローバル領域のサイズ、`enter_block_scope` を `i64` で checked に
   4. [ ] 各修正箇所にバグの原因と解決策のコメントを記載
   5. [ ] fails テスト・unit テスト追加、spec.md 追記、VERSION.md 追記
2. [ ] 02: 案 B は実施しない（記録のみ。[02-size-limit-option-b.md](02-size-limit-option-b.md)）
3. [ ] [08-array-size-overflow.md](../code-review-2026-10/08-array-size-overflow.md) の状態を更新

## 関連する潜在問題（本タスクでは扱わない）

- WS コンパイラで、ルートレベルのブロック内の変数がグローバル変数と同じアドレスに配置される → [ws-root-block-variable-address/](../../done-task/ws-root-block-variable-address/README.md) に記録

- `scan_variable_declarations`（`mod.rs:721`）は、型注釈 `@T[N]` の変数のスロット数を `N` としており、要素サイズ（struct なら `total_size`）を掛けていない。現状は `let: a@P[2];` が `statement.rs:286` で `type mismatch` になり配列型の変数を宣言できないため、顕在化しない。配列型の変数を許可するときは、スロット数を struct フィールドと同じ `value_type_size` で計算すること
