# token_parser モジュール変更設計

## 対象ファイル

- `src/token_parser/mod.rs`

## 現状

- `Token::Asterisk` は既に存在（乗算 `*` として使用中）
- `&` は `&&`（`Token::DoubleAmpersand`）の先頭文字として消費される
- 単独 `&` はエラー: `"single '&' is not supported yet"` (L340-L349)
- `Token::Ampersand` バリアントは存在しない

## 変更内容

### 1. Token enum に `Ampersand` を追加

```rust
pub enum Token {
    // ... 既存 ...
    DoubleAmpersand,  // &&
    DoublePipe,       // ||
    Ampersand,        // & (新規追加)
    // ...
}
```

### 2. `&` のパース処理を修正

現在のコード（L340-L349 付近）:

```rust
'&' => {
    // &&
    let next = chars_next!();
    if next == '&' {
        push_token!(Token::DoubleAmpersand, 2);
    } else {
        return code_parse_error!("single '&' is not supported yet");
    }
}
```

変更後:

```rust
'&' => {
    let next = chars_next!();
    if next == '&' {
        push_token!(Token::DoubleAmpersand, 2);
    } else {
        // 1文字戻る
        chars_back!();
        push_token!(Token::Ampersand, 1);
    }
}
```

注意: `chars_back!()` マクロまたは同等の処理が利用可能か確認が必要。既存のパーサで1文字先読み後に戻す処理があるかを確認する。

### 3. Token の Display 実装（もし存在する場合）

`Ampersand` に対応する表示文字列 `"&"` を追加。

## テスト

### ユニットテスト追加

```
& → Token::Ampersand
&& → Token::DoubleAmpersand（変更なし）
&x → Token::Ampersand, Token::Identifier("x")
```

## 影響範囲

- `*` トークンは既存の `Token::Asterisk` をそのまま使用（変更不要）
- tree_parser が新しい `Token::Ampersand` を受け取れるようにする必要がある
