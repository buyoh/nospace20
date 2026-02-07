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

**状態**: ⚠️ 仕様検討中

**説明**: `return:` がない場合、関数は値を返さない (`None`)。この挙動は要検討。

**現状**:
```nospace
func: foo() {
  1 + 2;  # return がないので値を返さない #
}
```

**選択肢**:

1. **現状維持**: return がない場合は値を返さない (None/void)
2. **暗黙の return**: 最後の式を返す (Rust スタイル)
3. **エラー**: return がない場合はエラー

**TODO**: 仕様を確定させる

**参照**:
- [spec.md](../../spec.md) セクション 5

**優先度**: 低 - 仕様の明確化が必要

---

## 実装の優先順位

1. **if/while 式の戻り値** - より表現力の高い言語のため
2. **return なし関数の戻り値** - 仕様の明確化
3. **文字リテラルの16進数表記** - 利便性向上
4. **数値の16進数リテラル** - 利便性向上

---

## 関連ドキュメント

- [spec.md](../../spec.md) - 言語仕様
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md) - 実装状況の詳細

---

## 更新履歴

- 2026-02-07: 文字リテラルの16進数表記 (`\xHH`) を追加
- 2026-02-07: unimplemented-features.md から分離して作成
