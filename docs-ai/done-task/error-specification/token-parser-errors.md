# 字句解析エラー (Token Parser Errors)

## 概要

字句解析フェーズでは、ソースコードを文字単位で読み取り、トークン列に変換する。このフェーズで検出されるエラーは、不正な文字や不正なリテラル形式に関するものである。

**実装場所**: `src/token_parser/mod.rs`  
**エラー型**: `CodeParseError`

## エラー一覧

### 1. 不正な16進数リテラル

#### 1.1 16進数桁が存在しない

**エラーメッセージ**:
```
invalid hexadecimal literal: expected at least one hex digit after '0x'
```

**発生条件**: `0x` の後に16進数文字（0-9, a-f, A-F）が1つも続かない

**テストケース**: `resources/tests/fails/syntax/hex_invalid_001.ns`

**ソースコード**: `src/token_parser/mod.rs:98-101`

```rust
if !has_digit {
    return Err(code_parse_error!(
        hex_idx,
        "invalid hexadecimal literal: expected at least one hex digit after '0x'"
    ));
}
```

**例**:
```nospace
let: x;
x = 0x;     # エラー
x = 0xG;    # エラー
x = 0x10;   # OK
```

---

### 2. 文字リテラルのエラー

#### 2.1 不完全な16進数エスケープシーケンス（1桁目）

**エラーメッセージ**:
```
incomplete hex escape sequence: expected 2 hex digits after '\x'
```

**発生条件**: `\x` の後に2桁の16進数が続かない（1桁目で終了）

**テストケース**: `resources/tests/fails/syntax/char_hex_invalid_001.ns`

**ソースコード**: `src/token_parser/mod.rs:143-147`

```rust
let hex1 = iter.next().ok_or_else(|| {
    code_parse_error!(
        hex1_idx,
        "incomplete hex escape sequence: expected 2 hex digits after '\\x'"
    )
})?.1;
```

---

#### 2.2 不完全な16進数エスケープシーケンス（2桁目）

**エラーメッセージ**:
```
incomplete hex escape sequence: expected 2 hex digits after '\x'
```

**発生条件**: `\x` の後に1桁しか続かない（2桁目で終了）

**テストケース**: `resources/tests/fails/syntax/char_hex_invalid_002.ns`

**ソースコード**: `src/token_parser/mod.rs:151-155`

```rust
let hex2 = iter.next().ok_or_else(|| {
    code_parse_error!(
        hex2_idx,
        "incomplete hex escape sequence: expected 2 hex digits after '\\x'"
    )
})?.1;
```

---

#### 2.3 不正な16進数エスケープシーケンス

**エラーメッセージ**:
```
invalid hex escape sequence: \x{hex_str}
```

**発生条件**: `\xHH` の HH 部分が16進数として解釈できない

**ソースコード**: `src/token_parser/mod.rs:162-166`

```rust
i64::from_str_radix(&hex_str, 16).map_err(|_| {
    code_parse_error!(
        hex1_idx,
        format!("invalid hex escape sequence: \\x{}", hex_str)
    )
})?
```

---

#### 2.4 未知のエスケープシーケンス

**エラーメッセージ**:
```
unknown escape sequence: \{c}
```

**発生条件**: サポートされていないエスケープシーケンス（`\n`, `\r`, `\t`, `\s`, `\\`, `\'`, `\xHH` 以外）

**ソースコード**: `src/token_parser/mod.rs:169-173`

```rust
Some((idx, c)) => {
    return Err(code_parse_error!(
        idx,
        format!("unknown escape sequence: \\{}", c)
    ));
}
```

**例**:
```nospace
let: x;
x = '\q';  # エラー: unknown escape sequence: \q
```

---

#### 2.5 文字リテラル中の予期しないファイル終端

**エラーメッセージ**:
```
unexpected end of input in character literal
```

**発生条件**: 文字リテラルが閉じられないままファイルが終了

**ソースコード**: 
- `src/token_parser/mod.rs:175-179` (エスケープシーケンス後)
- `src/token_parser/mod.rs:190-194` (文字後)

```rust
None => {
    return Err(code_parse_error!(
        start_idx,
        "unexpected end of input in character literal"
    ));
}
```

---

#### 2.6 空の文字リテラル

**エラーメッセージ**:
```
empty character literal
```

**発生条件**: `''` のように、文字リテラルが空

**ソースコード**: `src/token_parser/mod.rs:183-187`

```rust
Some((_, '\'')) => {
    return Err(code_parse_error!(
        start_idx,
        "empty character literal"
    ));
}
```

**例**:
```nospace
let: x;
x = '';  # エラー
```

---

#### 2.7 閉じられていない文字リテラル

**エラーメッセージ**:
```
expected closing quote, found: {c}
```

**発生条件**: 文字リテラルが `'` で閉じられていない

**別メッセージ**:
```
unclosed character literal
```

**ソースコード**: 
- `src/token_parser/mod.rs:200-204` (予期しない文字)
- `src/token_parser/mod.rs:205-209` (EOF)

```rust
Some((idx, c)) => Err(code_parse_error!(
    idx,
    format!("expected closing quote, found: {}", c)
)),
None => Err(code_parse_error!(
    start_idx,
    "unclosed character literal"
)),
```

---

### 3. 不正な文字

#### 3.1 認識できない文字

**エラーメッセージ**:
```
invalid char: {c}
```

**発生条件**: トークンとして認識できない文字が出現

**テストケース**: `resources/tests/fails/syntax/invalid_token_001.ns`

**ソースコード**: `src/token_parser/mod.rs:371`

```rust
parse_errors.push(code_parse_error!(*idx, format!("invalid char: {}", c)));
```

**例**:
```nospace
func: main() {
  @ invalid  # エラー: invalid char: @
}
```

---

#### 3.2 変換失敗（トークン化エラー）

**エラーメッセージ**:
```
failed to convert to token: {token:?}
```

**発生条件**: トークン変換中の内部エラー（通常は発生しない）

**ソースコード**: `src/token_parser/mod.rs:348-351`

```rust
parse_errors.push(code_parse_error!(
    *idx,
    format!("failed to convert to token: {:?}", token)
));
```

---

## サポートされているエスケープシーケンス

| シーケンス | 意味 | ASCII コード |
|-----------|------|------------|
| `\n` | 改行 (LF) | 10 |
| `\r` | 復帰 (CR) | 13 |
| `\t` | タブ | 9 |
| `\s` | スペース | 32 |
| `\\` | バックスラッシュ | 92 |
| `\'` | シングルクォート | 39 |
| `\xHH` | 16進数2桁 | 0-255 |

## テストケースの網羅性

現在のテストケース：

| テストケース | パス | カバーしているエラー |
|------------|------|-------------------|
| `hex_invalid_001.ns` | `fails/syntax/` | 不正な16進数リテラル（G は16進数文字でない） |
| `char_hex_invalid_001.ns` | `fails/syntax/` | 不完全な16進数エスケープシーケンス（1桁目） |
| `char_hex_invalid_002.ns` | `fails/syntax/` | 不完全な16進数エスケープシーケンス（2桁目） |
| `invalid_token_001.ns` | `fails/syntax/` | 不正な文字 (@) |

### 不足しているテストケース

- [ ] 未知のエスケープシーケンス (`\q` など)
- [ ] 空の文字リテラル (`''`)
- [ ] 閉じられていない文字リテラル (`'a`)
- [ ] 文字リテラル中の予期しないファイル終端
- [ ] 16進数リテラル `0x` のみで終了
- [ ] トークン変換失敗（内部エラー、通常は発生しない）
