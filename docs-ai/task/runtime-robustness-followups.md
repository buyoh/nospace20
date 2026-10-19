# 実行時エラー・ネスト深度対策の残課題

[runtime-robustness-2026-10](../done-task/runtime-robustness-2026-10/README.md)（コードレビュー 2026-10 の 05 / 07 / 12）の実装・レビューで判明し、範囲外とした項目。

作成日: 2026-10-19
ステータス: 未着手

## 背景・動機

05 / 07 / 12 の対応で CLI・WASM の主要な panic / abort 経路は解消したが、以下は規模や方針決定が必要なため分離した。

## 設計・方針（項目ごと）

### 1. インタプリタのヒープ総量に上限がない（minor）

- `__alloc` は 1 回あたり `MAX_ALLOC_WORDS`（16M ワード）までだが、総量は無制限。`while: 1 { __alloc(16777216); }` で 128MiB ずつ確保し、最終的にホストのメモリ確保失敗で abort（WASM は trap）する
- 注（2026-10-19）: 製品の実行経路は [retire-direct-interpreter](retire-direct-interpreter/README.md) で WhitespaceVM に置き換わり、上限は [ws-vm-run-mode-support 01](ws-vm-run-mode-support/01-memory-limit.md) のメモリ上限が担う。retire の完了後は本項目は不要になる
- 案: `InterpreterAllocator` に生存ワード数の上限（例: 64M ワード）を持たせ、超過時は `InterpretError::OutOfMemory`（新設）を返す。上限値は `EnvironmentConfig` で設定可能にし、spec.md の Appendix に記載

### 2. ネスト深度の管理が 2 つの builder に重複（minor）

- `ExpressionBuilder` / `StatementBuilder` の `depth` / `peak` と上限確認（`nested` / `parse_nested_statements` / `add_nesting_error`）、`parse_type_spec` が重複している
- `NESTING_TOO_DEEP_MESSAGE` は `MAX_NESTING_DEPTH` の値を手書きしている（一致はユニットテストで確認）
- 案: `NestingTracker { depth, peak }` を切り出して両 builder で共有する。[17-tree-parser-duplication.md](code-review-2026-10/17-tree-parser-duplication.md) の `ParserCursor` 抽出と同時に行う

### 3. 上限付近の入力が実際に 1MiB スタックで処理できることの自動テストがない（minor）

- 現状は release ビルドを `ulimit -s 1024` で手動確認した（README の進捗参照）
- `cargo test` は debug ビルドで、意味解析のフレームが release の 10 倍以上あるため、`std::thread::Builder::stack_size` を使うテストは debug 基準のサイズにする必要がある。CI で release ビルドの CLI を `ulimit -s 1024` で実行するスクリプトの方が実態に近い

### 4. 意味解析の式変換関数の debug ビルドでのスタックフレーム肥大（minor）

- `semantic_analyzer::expression::convert_to_exec_expression_with_resolver`（約 580 行）が debug で約 27KB/段（release は 1〜2KB/段）。上限 128 の範囲でも debug ビルドでは 250 項程度の演算子連鎖で main スレッド（8MB）を使い切る
- 案: match の各分岐を個別の関数に分割してフレームを小さくする

### 5. ツリー走査インタプリタ（`interpreter/exec.rs`）の扱い（05-A）

- CLI は NospaceVM に移行済み。`exec.rs` は `lib.rs` の `interpret_*` API とテストの `interpreter` ターゲットからのみ使われ、実行時エラーを panic で表現している
- 案: テストのツリー走査インタプリタ用ターゲットを NospaceVM に寄せ、`exec.rs` と `interpret_*` API を廃止する（組み込み関数等の意味論の二重実装も解消）

### 6. `WasmWhitespaceVM::new` が最適化パスを受け取らない（12 の問題点の残り）

- 注（2026-10-19）: [ws-vm-run-mode-support 03](ws-vm-run-mode-support/03-wasm-whitespace-vm.md) で解消する予定

### 7. 代入できない左辺が意味解析を通過する（major、本タスク以前からの不具合）

- `x + 1 = 1;`・`-x = 1;`・`g() = 1;`・`{ x; } = 1;`・`(x)@int = 1;` などが意味解析を通過する（`&(x+1)` は意味解析でエラーになるのと非対称）
- 構造体フィールドへの代入 `s.f = 5;`・`s.f[1] = 5;` は Whitespace コンパイラでは対応済みだが、NospaceVM・ツリー走査インタプリタでは未対応
- 本タスクでは NospaceVM の panic を `InterpretError::NotAssignable` に変更して abort を防いだのみ（ツリー走査インタプリタは panic のまま）
- 案: 意味解析で左辺値を検査して compile error にする。フィールド代入は NospaceVM に実装する（または全バックエンドで一貫して拒否）。`fails/semantic/` の large テストとフィールド代入の passes テストを追加

### 8. 配列型サイズの乗算オーバーフロー

- `struct: S(x@int[65536][65536][65536][65536][65536]);` で `semantic_analyzer/mod.rs` のサイズ計算が debug でオーバーフロー panic、release では誤ったサイズになる
- [08-array-size-overflow.md](code-review-2026-10/08-array-size-overflow.md) で対応する

## タスクステップ・チェックリスト

1. [ ] ヒープ総量上限
2. [ ] `NestingTracker` 抽出（17 と同時）
3. [ ] 1MiB スタックでの自動確認（CI スクリプト）
4. [ ] 意味解析の式変換関数の分割
5. [ ] `exec.rs` の廃止
6. [ ] `WasmWhitespaceVM::new` の最適化パス対応
7. [ ] 左辺値の意味解析検査・フィールド代入のインタプリタ対応
