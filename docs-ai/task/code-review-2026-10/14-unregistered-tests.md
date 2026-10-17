# テストリソースにどこからも実行されないテストが残っている

- 重要度: **medium**
- 状態: 未着手

## 進捗

- [ ] hex 系テストの manifest 登録
- [ ] 旧形式テストの移行 or 削除
- [ ] コメントアウトテストの扱い決定
- [ ] 未登録検出チェックの導入

## 問題点

`resources/tests/test-manifest.yaml` にも Rust コードにも参照のない `.ns` がある。

1. **機能実装済み・手元では通るのに未登録**
   - `passes/literals/char_hex_001`、`passes/literals/hex_number_001`
   - `fails/syntax/char_hex_invalid_001`、`char_hex_invalid_002`、`hex_invalid_001`
   - → 16 進リテラル機能の回帰が検出されない
2. **旧形式で実行系がない**
   - `passes/main_idx/main_function_index_00{1,2}.ns` + `.expected`
   - `passes/builtins/builtin_all_test.ns`
3. **manifest でコメントアウトされたまま放置**（`[TODO: ENABLE]` 等）
   - `legacy_018/019/021/022/024/026`、`func_nested_001`、`disabled_var_*` など約 13 件
   - 例: `legacy_026` は現在 tokenize エラーになるが追跡されていない

また `src/whitespace/parser_tests.rs` / `interpreter_tests.rs` にオーバーフロー・`i64::MIN`・不正ラベルの境界値テストがない（[03](03-ws-vm-arithmetic-overflow.md)・[04](04-ws-parser-number-label.md) が見逃された原因）。

## 違反している原則

- テスト品質（重要経路のカバレッジ）

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | 未登録 hex 系テストを manifest に登録。`main_idx` は check.json 形式に移行するか削除 | manifest と resources のみ |
| B | `src_build` に「manifest 未登録の `.ns` があれば build 時 warning」チェックを追加 | `src_build/nospace_tests.rs` |
| C | コメントアウトテストは `docs-ai/task` で追跡するか、`exclude_targets` / ignore 相当の明示的状態に移す | manifest、`src_build` |

推奨: A + B + C。
