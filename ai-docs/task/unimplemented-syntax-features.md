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

**状態**: ❌ 未実装

**説明**: 16進数リテラル (`0x...`) は未対応。現在は10進整数のみサポート。

**構文例**:
```nospace
let:x = 0xFF;   # 未実装 (255になるべき) #
let:y = 0x10;   # 未実装 (16になるべき) #
```

**実装箇所**: [src/token_parser/mod.rs](../../src/token_parser/mod.rs)

**コード**:
```rust
// TODO: 0x
```

**実装に必要な変更**:

1. **トークンパーサ**:
   - `0x` プレフィックスを検出
   - 16進数文字列を解析
   - i64 に変換

**参照**:
- [spec.md](../../spec.md) セクション 1.1

**優先度**: 低 - 利便性向上だが、10進数で代用可能

---

### 1.2 文字リテラルの16進数表記

**状態**: ❌ 未実装

**説明**: 文字リテラル内での16進数エスケープシーケンス (`\xHH`) は未対応。現在は `\n`, `\t`, `\s` 等の特定エスケープシーケンスのみサポート。

**構文例**:
```nospace
let:x = '\x41';   # 未実装 (65, 'A' になるべき) #
let:y = '\x0A';   # 未実装 (10, 改行 になるべき) #
let:z = '\x20';   # 未実装 (32, スペース になるべき) #
```

**現状の代替手段**:
```nospace
let:x = 'A';    # 65 #
let:y = '\n';   # 10 (改行) #
let:z = '\s';   # 32 (スペース) #
```

**実装箇所**: [src/token_parser/mod.rs](../../src/token_parser/mod.rs#85-134)

**現在のコード**:
```rust
fn parse_char_literal<I: Iterator<Item = (usize, char)>>(
    iter: &mut iter::Peekable<I>,
    start_idx: usize,
) -> Result<Token, CodeParseError> {
    let char_value = match iter.next() {
        Some((_, '\\')) => {
            // エスケープシーケンス
            match iter.next() {
                Some((_, 'n')) => 10,  // 改行 (LF)
                Some((_, 'r')) => 13,  // 復帰 (CR)
                Some((_, 't')) => 9,   // タブ
                Some((_, 's')) => 32,  // スペース
                Some((_, '\\')) => 92, // バックスラッシュ
                Some((_, '\'')) => 39, // シングルクォート
                // \x はここに追加が必要
                Some((idx, c)) => {
                    return Err(code_parse_error!(
                        idx,
                        format!("unknown escape sequence: \\{}", c)
                    ));
                }
                // ...
            }
        }
        // ...
    };
}
```

**実装に必要な変更**:

1. **トークンパーサ** (`parse_char_literal` 関数):
   - `\x` を検出
   - 次の2文字を16進数として解析 (0-9, A-F, a-f)
   - 16進数を i64 に変換
   - エラーハンドリング (不正な16進数文字、桁数不足など)

**実装例**:
```rust
Some((_, 'x')) => {
    // \xHH 形式の16進数エスケープ
    let hex1 = iter.next().ok_or_else(|| code_parse_error!(...))?.1;
    let hex2 = iter.next().ok_or_else(|| code_parse_error!(...))?.1;
    let hex_str = format!("{}{}", hex1, hex2);
    i64::from_str_radix(&hex_str, 16).map_err(|_| code_parse_error!(...))?
}
```

**参照**:
- [spec.md](../../spec.md) セクション 1.3 (文字リテラル)

**優先度**: 低 - 利便性向上だが、既存のエスケープシーケンスや直接文字指定で代用可能

---

## 2. if/while 式の戻り値

**状態**: ⚠️ 制限あり

**説明**: if と while は式として使用可能だが、常に 0 を返す。将来的には評価した値を返すように改善予定。

**現状**:
```nospace
x = if: cond { 5 } else: { 10 };  # x は常に 0 #
```

**期待される動作**:
```nospace
x = if: cond { 5 } else: { 10 };  # x は cond が真なら 5、偽なら 10 #
```

**TODO**: 評価した値を返すようにする

**実装に必要な変更**:

1. **インタプリタ**:
   - if/while の評価結果を保持
   - ブロックの最後の式の値を返す
   
2. **意味解析器**:
   - ブロックの最後の式を特定
   - 戻り値の型チェック (将来の型システム導入時)

**参照**:
- [spec.md](../../spec.md) セクション 6.1, 6.2
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

**優先度**: 中 - より表現力の高い言語のため

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
- [spec.md](../../spec.md) セクション 5
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

- [spec.md](../../spec.md) - 言語仕様
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md) - 実装状況の詳細

---

## 更新履歴

- 2026-02-07: return なし関数の戻り値の仕様を更新 (0 を返すと確定)
- 2026-02-07: 文字リテラルの16進数表記 (`\xHH`) を追加
- 2026-02-07: unimplemented-features.md から分離して作成
