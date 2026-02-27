# 未実装の構文と式の機能

このドキュメントは nospace プログラミング言語における未実装の構文と式の機能をまとめたものです。

最終更新日: 2026-02-07

## 目次

1. [16進数リテラル](#1-16進数リテラル)
   - 1.1 [数値の16進数リテラル](#11-数値の16進数リテラル)
   - 1.2 [文字リテラルの16進数表記](#12-文字リテラルの16進数表記)
2. [if/while 式の戻り値](#2-ifwhile-式の戻り値)
3. [return なし関数の戻り値](#3-return-なし関数の戻り値)

---

## 1. 16進数リテラル

### 1.1 数値の16進数リテラル

**状態**: ✅ 実装済み

**説明**: 16進数リテラル (`0x...`) をサポート。

**構文例**:
```nospace
let:x = 0xFF;   # 255 #
let:y = 0x10;   # 16 #
```

**実装箇所**: [src/token_parser/mod.rs](../../src/token_parser/mod.rs)

**テストケース**:
- [resources/tests/passes/literals/hex_number_001.ns](../../resources/tests/passes/literals/hex_number_001.ns)
- [resources/tests/fails/syntax/hex_invalid_001.ns](../../resources/tests/fails/syntax/hex_invalid_001.ns)

**実装日**: 2026-02-07

---

### 1.2 文字リテラルの16進数表記

**状態**: ✅ 実装済み

**説明**: 文字リテラル内での16進数エスケープシーケンス (`\xHH`) をサポート。

**構文例**:
```nospace
let:x = '\x41';   # 65, 'A' #
let:y = '\x0A';   # 10, 改行 #
let:z = '\x20';   # 32, スペース #
```

**実装箇所**: [src/token_parser/mod.rs](../../src/token_parser/mod.rs)

**テストケース**:
- [resources/tests/passes/literals/char_hex_001.ns](../../resources/tests/passes/literals/char_hex_001.ns)
- [resources/tests/fails/syntax/char_hex_invalid_001.ns](../../resources/tests/fails/syntax/char_hex_invalid_001.ns)
- [resources/tests/fails/syntax/char_hex_invalid_002.ns](../../resources/tests/fails/syntax/char_hex_invalid_002.ns)

**実装日**: 2026-02-07

---

## 2. if/while 式の戻り値

**状態**: ✅ 実装済み

**説明**: if と while が式として評価した値を返すようになりました。

**実装済みの動作**:
```nospace
x = if: cond { 5; } else: { 10; };  # x は cond が真なら 5、偽なら 10 #
y = while: i - 3 { i = i + 1; i; };  # y は最後のイテレーションの最後の式の値 #
```

**仕様**:
- **if 式**: 実行されたブロック(then または else)の最後の式の値を返す
- **while 式**: 
  - 通常終了: 最後のイテレーションの最後の式の値を返す
  - ループが一度も実行されない場合: 0 を返す
  - break で終了した場合: 0 を返す
  - continue: 通常のイテレーションとして処理

**実装箇所**: 
- [src/interpreter/mod.rs](../../src/interpreter/mod.rs) - interpret_if, interpret_while, interpret_statements_with_value
- [src/tree_parser/expression/mod.rs](../../src/tree_parser/expression/mod.rs) - parse_to_expression_tree_factor

**テストケース**:
- [resources/tests/passes/control_flow/if_expr_value_001.ns](../../resources/tests/passes/control_flow/if_expr_value_001.ns)
- [resources/tests/passes/control_flow/while_expr_value_001.ns](../../resources/tests/passes/control_flow/while_expr_value_001.ns)

**参照**:
- [docs/spec.md](../../docs/spec.md) セクション 6.1, 6.2
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

---

## 3. return なし関数の戻り値

**状態**: ✅ 仕様確定

**説明**: `return:` がない場合、関数は 0 を返す。

**現状の仕様**:
```nospace
func: foo() {
  1 + 2;  # return がないので 0 を返す #
}

func: main() {
  let:x;
  x = foo();
  __assert(x == 0);  # foo() は 0 を返す #
}
```

**将来の変更予定**:
- 型システム導入後、void 型以外の関数で return がない場合はエラーとなる予定
- void 型の関数では引き続き暗黙的に終了（値を返さない）

**参照**:
- [docs/spec.md](../../docs/spec.md) セクション 5
  - 「`return:` がない場合、0を返す。」
  - 「TODO: 型実装後、void 以外ではエラーとなる。」

**優先度**: なし - 仕様確定済み (将来の型システム導入時に再検討)

---

## 実装の優先順位

1. **if/while 式の戻り値** - より表現力の高い言語のため
2. **文字リテラルの16進数表記** - 利便性向上
3. **数値の16進数リテラル** - 利便性向上

注: return なし関数の戻り値は仕様確定済み (0 を返す)

---

## 関連ドキュメント

- [docs/spec.md](../../docs/spec.md) - 言語仕様
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md) - 実装状況の詳細

---

## 更新履歴

- 2026-02-07: return なし関数の戻り値の仕様を更新 (0 を返すと確定)
- 2026-02-07: 文字リテラルの16進数表記 (`\xHH`) を追加
- 2026-02-07: unimplemented-features.md から分離して作成
