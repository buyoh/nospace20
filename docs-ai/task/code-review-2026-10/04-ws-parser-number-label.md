# WS パーサ: 数値リテラルのオーバーフローで panic、`i64::MIN` が往復しない、ラベル同一性の誤り

- 重要度: **high**
- 状態: 未着手

## 進捗

- [ ] 数値パースの checked 化
- [ ] ラベル表現の見直し
- [ ] 境界値テスト追加

## 問題点

`src/whitespace/parser.rs`

1. **数値オーバーフローで panic**（`:230,234` の `value * 2` / `value * 2 + 1`）。64 ビットを超えるビット列（外部から与えた ws ファイル）で `panicked at src/whitespace/parser.rs:234:25`。release では wrap。
2. **`i64::MIN` が往復しない**（`:250` の `-value`）。`WsNumber::encode`（`base/ws_types.rs:34-64`）は `i64::MIN` を正しく出力するが、パーサが正の値として累積するため読めない。`--opt all` で `-9223372036854775807 - 1` が畳み込まれると `push -9223372036854775808` を含む ws が生成され、whitespace20 が `parser.rs:230` で panic する。**自身のコンパイラ出力を自身の VM で実行できない。**
3. **ラベル同一性の誤り**（`:259-262`）。`LabelId(number.0.unsigned_abs() as u32)` のため、Whitespace では別ラベルである `[S,T]`・`[T,T]`・`[S,S,T]` がすべてラベル 1 になる。32 ビット超のラベルも衝突する。
   - 再現: `printf '\n   \t\n\n  \t\t\n\n\n\n'`（ラベル ST と TT を定義）→ whitespace20: `DuplicateLabel { label_id: 1 }`、wsc: 正常終了
   - 他ツールが生成した正当なプログラムがエラーになるか、誤ったラベルへジャンプする

## 違反している原則

- 信頼できない入力で panic しない（AGENTS.md の panic 方針）
- Whitespace 仕様準拠
- encode / parse の往復性

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `parse_number` を `checked_mul` / `checked_add` で実装し、失敗時 `ParseError::NumberOverflow`。符号を先に適用して負方向に累積すれば `i64::MIN` も表現可能 | `whitespace/parser.rs`、`base/error/ws_error.rs` |
| B | ラベルをビット列（`Vec<bool>` または `(len, bits)`）のまま扱い、パース時に連番 ID へ写像する | `whitespace/parser.rs`、`WhitespaceVM::labels`。`base::ws_types::LabelId` はコンパイラと共有のため、パーサ専用型の分離が必要 |
| C | `parser_tests.rs` に境界値テスト（`i64::MIN/MAX` 往復、65 ビット以上、ラベル衝突）を追加 | テストのみ |

推奨: A + B + C すべて。
