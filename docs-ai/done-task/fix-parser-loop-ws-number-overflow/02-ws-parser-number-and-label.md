# 02: WS パーサの数値 checked 化とラベルのビット列による同一性判定

Whitespace パーサ（`src/whitespace/parser.rs`）で、64 ビットに収まらない数値をエラーとして返し、`i64::MIN` を正しく読めるようにする。あわせて、ラベルを数値ではなくビット列として同一性判定し、連番の `LabelId` に写像する。

作成日: 2026-10-18
ステータス: 完了

## 背景・動機

[code-review-2026-10/04](../../task/code-review-2026-10/04-ws-parser-number-label.md) 参照。採用案は **A + B + C**。

| 問題 | 原因 | 対応 |
|---|---|---|
| 数値オーバーフローで panic（release では wrap） | `parse_number` の `value * 2` / `value * 2 + 1` が未チェック | A: checked 演算にしてエラーを返す |
| `i64::MIN` が往復しない | 正方向に累積してから `-value` するため、絶対値 2^63 を表現できない | A: 負方向に累積する |
| 別ラベルが同一視される | `parse_label` が数値としてパースし `unsigned_abs() as u32` に変換するため、先頭文字を符号として捨て、先頭の 0 も無視し、32 ビット超は切り捨てる | B: ビット列を key にして連番 ID に写像する |

Whitespace のラベルは「Space / Tab の任意長の列 + LF」であり、符号も数値としての意味も持たない。`[S,T]`・`[T,T]`・`[S,S,T]`・`[]`（空ラベル）はすべて別ラベルである。

## 設計・方針

### A: `parse_number` の checked 化

負方向に累積し、最後に正なら符号反転する。`i64` の負方向の範囲は正方向より 1 広いので、`i64::MIN`（絶対値 2^63）も表現できる。

```rust
fn parse_number(chars: &[WsChar], pos: usize) -> Result<(WsNumber, usize), ParseError> {
    // 符号の読み取りは現状維持
    // バグ修正: 従来は正方向に未チェックで累積していたため、64 ビットを超えるビット列で
    // panic（debug）/ wrap（release）し、絶対値 2^63 の i64::MIN も読めなかった。
    // 解決策: 負方向に checked 演算で累積し、正の数のときだけ最後に checked_neg する。
    let overflow = || ParseError::NumberOverflow { position: pos };
    let mut value: i64 = 0; // 常に 0 以下（絶対値の符号反転）
    loop {
        let bit = match chars.get(current) {
            Some(WsChar::Space) => 0,
            Some(WsChar::Tab) => 1,
            Some(WsChar::Lf) => { current += 1; break; }
            None => return Err(UnexpectedEof { .. }),
        };
        value = value.checked_mul(2).and_then(|v| v.checked_sub(bit)).ok_or_else(overflow)?;
        current += 1;
    }
    let value = if negative { value } else { value.checked_neg().ok_or_else(overflow)? };
    Ok((WsNumber(value), current))
}
```

- 先頭に 0 ビット（Space）がいくら続いても値が 0 のままなので、オーバーフローしない（wsc と同じく受理）
- `+2^63`（正符号 + 1 の後に 0 が 63 個）は `checked_neg` で失敗しエラーになる
- `position` は数値リテラルの開始位置（`parse` 内部の WsChar 列でのインデックス。既存の `InvalidCommand { position }` と同じ基準）

#### エラー型

`WsParseError`（`src/base/error/ws_error.rs`）に `NumberOverflow { position: usize }` を追加する。

- 既存の `InvalidNumber { position }` は未使用だが、「形式が不正」と「値が範囲外」はユーザへの対処が異なるため別バリアントにする。`InvalidNumber` の扱い（削除など）は [19-dead-code.md](../../task/code-review-2026-10/19-dead-code.md) に委ねる
- `Display`: `"number at position {} is out of the 64-bit signed integer range"`
- 網羅的 match の更新が必要な箇所: `tests/whitespace_direct_test/base.rs` の `test_ws_parse_error_base`（`ParseError::NumberOverflow { .. } => "NumberOverflow"` を追加）、`ws_error_tests.rs`（Display のテスト追加）。`whitespace20` と WASM（`wasm_api/whitespace_vm.rs`）は `Display` / `Debug` 表示のみで変更不要

### B: ラベルをビット列で同一性判定し、連番 `LabelId` に写像する

`LabelId(u32)` と `Instruction` はコンパイラ（`compiler_ws`）と共有の型（`base::ws_types`）なので、**型は変更せず、パーサ内でビット列 → `LabelId` の写像（interning）を行う**。

- 同じビット列には同じ `LabelId`、異なるビット列には異なる `LabelId` を割り当てる（出現順に 0, 1, 2, ...）
- `Call` / `Jump` が `Label` より先に出現しても、同じ写像表を共有するので同じ ID になる
- `WhitespaceVM` の `labels: HashMap<i64, usize>`・`collect_labels`・`resolve_label` は `LabelId` 単位で動くため**変更不要**。重複ラベル検出も、写像後の ID が同じ = ビット列が同じなので正しく動く
- これにより、レビュー記録で言及していた「パーサ専用ラベル型の分離」は不要になる

#### 写像表の構造体

単一責任のため、写像表は独立した構造体にする（`src/whitespace/label_interner.rs` を新設。モジュール外には公開しない）。

```rust
/// Whitespace のラベル（Space/Tab のビット列）を連番の LabelId に写像する表
///
/// Whitespace のラベルは符号を持たない任意長のビット列であり、数値として解釈すると
/// `[S,T]` と `[T,T]` のような別ラベルが衝突する。ビット列そのものを key にして区別する。
pub(super) struct LabelInterner<'a> {
    /// ビット列（LF を含まない）→ 割り当て済み ID
    ids: HashMap<&'a [WsChar], LabelId>,
}

impl<'a> LabelInterner<'a> {
    pub(super) fn new() -> Self;
    /// 既知のビット列なら既存 ID、未知なら次の連番を割り当てて返す。
    /// ID が u32 に収まらない場合（ラベル数 > u32::MAX）は None
    pub(super) fn intern(&mut self, bits: &'a [WsChar]) -> Option<LabelId>;
}
```

- key は `parse` 内で作る `chars: Vec<WsChar>` のスライスを借用する（コピー不要）。そのため `WsChar`（`base/ws_types.rs`）に `Hash` を derive する
- ID が `u32` に収まらない場合は既存の未使用バリアント `WsParseError::InvalidLabel { position }` を返す（実際には入力サイズ上到達しないが、panic しないため）。これにより `InvalidLabel` は使用中になる（`InvalidNumber` は未使用のまま。死にコード整理時に `InvalidLabel` を削除しないこと）

#### パーサ関数の変更

- `parse` で `LabelInterner::new()` を作り、`parse_instruction` → `parse_flow` → `parse_label` に `&mut LabelInterner` を渡す（他の `parse_*` 関数のシグネチャは変更しない）
- `parse_label` は `parse_number` を使わず、LF までのビット列（Space/Tab）をスライスとして切り出して `intern` する。EOF は `UnexpectedEof { context: "label" }`

```rust
/// ラベルリテラルをパース
///
/// フォーマット: [Space/Tab のシーケンス][LF]（符号なし、長さ任意、空列も可）
// バグ修正: 従来は数値としてパースして unsigned_abs() as u32 に変換していたため、
// 先頭文字が符号として捨てられ [S,T] と [T,T] が同じラベルになり、
// 32 ビット超のラベルも衝突していた。
// 解決策: ビット列そのものを LabelInterner で連番 ID に写像する。
fn parse_label<'a>(
    chars: &'a [WsChar],
    pos: usize,
    labels: &mut LabelInterner<'a>,
) -> Result<(LabelId, usize), ParseError>
```

#### 影響と許容する変化

| 項目 | 変化 | 判断 |
|---|---|---|
| コンパイラ出力を再パースした場合のラベル番号 | コンパイラの `LabelId` とは別の連番になる | 実行結果には影響しない。パース結果の `LabelId` 値に依存するコードは無い（`examples/ws_profiler.rs` は命令種別ごとの回数のみ使用、確認済み） |
| `UndefinedLabel(i64)` / `DuplicateLabel { label_id }` のエラー表示 | 元のビット列ではなく写像後の ID が表示される | 許容。`DuplicateLabel` は命令位置も表示する。元のビット列を表示したい場合は別タスク（下記「残課題」） |
| WSA テスト `fails/parse/duplicate_label_001`・`fails/runtime/undefined_label_001` | 判定結果は不変（同じビット列は同じ ID、未定義は未定義のまま） | 回帰確認のみ |

### C: 境界値テスト

[タスクステップ](#タスクステップチェックリスト) のステップ 4・5 を参照。

## タスクステップ・チェックリスト

1. [x] A: `WsParseError::NumberOverflow` 追加（Display・`ws_error_tests.rs`・`tests/whitespace_direct_test/base.rs` のマッピング）
2. [x] A: `parse_number` を負方向 checked 累積に変更（バグ修正コメント付き）
3. [x] B: `WsChar` に `Hash` を derive、`label_interner.rs` 新設、`parse_label` をビット列写像に変更（バグ修正コメント付き）
4. [x] C: ユニットテスト追加（`parser_tests.rs`、`label_interner` のテスト）
5. [x] C: large テスト追加（`resources/tests_ws/`、`resources/tests/passes/`）
6. [x] `resources/tests_ws/README.md` を更新: `ws_parse_error` テスト形式の節を新設してエラー種別（`DuplicateLabel`・`NumberOverflow`・`InvalidCommand`・`UnexpectedEof` など）を列挙、数値は 64 ビット符号付き範囲に制限される旨（wsc との差異）を記載、ディレクトリ構造に既存の `fails/parse/` を追記
7. [x] `VERSION.md` の WIP セクションに変更内容を追記（例: `whitespace20: Distinguish labels by their exact Space/Tab sequence`、`whitespace20: Reject numbers outside the 64-bit range instead of panicking`）

### ステップ 4: ユニットテスト

`src/whitespace/parser_tests.rs`:

| テスト | 内容 | 期待 |
|---|---|---|
| `i64::MAX` 往復 | `WsNumber(i64::MAX).encode()` を `parse_number` | `i64::MAX` |
| `i64::MIN` 往復 | `WsNumber(i64::MIN).encode()` を `parse_number` | `i64::MIN` |
| 正の 2^63 | 正符号 + `T` + `S`×63 + LF | `Err(NumberOverflow)` |
| 65 ビット | 正/負符号 + `T`×65 + LF | `Err(NumberOverflow)`（panic しない） |
| 先頭 0 が長い | 正符号 + `S`×100 + `T` + LF | `1` |
| ラベル衝突の解消 | `Label[S,T]`・`Label[T,T]`・`Label[S,S,T]`・`Label[]` | 4 つの異なる `LabelId` |
| 前方参照 | `Call[T,S]` → `Label[T,S]` | 同じ `LabelId` |
| 長いラベル | `Label` に `T`×100 | panic せず `Ok` |
| 往復（Push） | `Instruction::Push(WsNumber(i64::MIN))` を encode → `parse` | 同じ命令 |

`src/whitespace/label_interner_tests.rs`（`#[path]` で分離する既存方式）: 同一ビット列で同一 ID、異なるビット列で異なる ID、空列も 1 つのラベルとして扱えること。

`WhitespaceVM::from_source` のテスト（`interpreter_tests.rs`）: レビュー記録の再現入力 `"\n   \t\n\n  \t\t\n\n\n\n"`（ラベル ST と TT を定義して exit）が `DuplicateLabel` にならず `Complete` すること。

### ステップ 5: large テスト

`resources/tests_ws/`（`ws_io` には wsc クロスバリデーションテストも自動生成される。`ws_io` の 3 件は wsc と結果が一致することを確認済み。`ws_parse_error` には wsc テストは生成されない）:

| ファイル | type | 内容 |
|---|---|---|
| `passes/stack/push_i64_min_001.wsa` | `ws_io` | `push -9223372036854775808` → `pnum` → stdout `-9223372036854775808` |
| `passes/stack/push_i64_max_001.wsa` | `ws_io` | `push 9223372036854775807` → `pnum` |
| `passes/flow/label_distinct_bits_001.wsa` | `ws_io` | ラベル `ST`・`TT`・`SST` を定義し、それぞれへのジャンプで異なる文字を出力 |
| `fails/parse/number_overflow_001.wsa` | `ws_parse_error` | 65 ビットの push。`"error": "NumberOverflow"`。**wsc（多倍長）はこの入力を受理して `36893488147419103231` を出力する**。whitespace20 が 64 ビット範囲外の数値を拒否するのは意図した制限であり、`resources/tests_ws/README.md` と spec.md の Appendix（[03](03-ws-vm-wrapping-arithmetic.md) で追加する処理系の挙動の節）に明記する |

`resources/tests/passes/`（nospace の large テスト）:

| ファイル | 内容 |
|---|---|
| `int_min_literal_fold_001.ns` | `__puti(-9223372036854775807 - 1);`（`success_io`、stdout `-9223372036854775808`）。`--opt all`（whitespace-self-opt-all ターゲット）で `push -9223372036854775808` が生成され、自身の VM で実行できることの回帰テスト |

注意: 旧実装は release ビルドでは `value * 2` が `i64::MIN` に wrap し、`-value` でも `i64::MIN` のままになるため、偶然 `i64::MIN` を正しく読める。`int_min_literal_fold_001` は debug ビルド（通常の `cargo test`）でのみ回帰を検出する。確実な回帰テストはステップ 4 の `parse_number` ユニットテスト（`i64::MIN` 往復・2^63・65 ビット）である。

## 残課題（本タスク範囲外）

- エラー表示で元のラベルビット列を示す（`LabelInterner` の逆引き表を VM に渡す必要があり、`from_instructions` の API に影響するため見送り）
