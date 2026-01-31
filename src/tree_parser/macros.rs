//! # Tree Parser Macros
//!
//! 構文解析で共通利用するマクロ定義

/// 期待するトークンなら Ok を返すマクロ
///
/// そうでなければ、Expression::Invalid に渡すべき CodeParseError のインデックスを返す。
///
/// # 注意
///
/// `iter.next()` と `iter.peek()` の選択について:
/// - `peek` は値を消費しない為に loop に陥る可能性がある
/// - この判定は難しいので、`next` を推奨する
macro_rules! match_expect_token {
    ($self: expr, $v: expr, $pat: pat) => {
        match $v {
            Some(($pat, _)) => Ok(()),
            Some((_, token_info)) => Err($self.add_parse_error(
                token_info,
                format!("unexpected token: expected {}", stringify!($pat)),
            )),
            None => Err($self.add_end_error("unexpected end of input".to_owned())),
        }
    };
    ($self: expr, $v: expr, $pat: pat if $cond:expr) => {
        match $v {
            Some(($pat, _)) if $cond => Ok(()),
            Some((_, token_info)) => Err($self.add_parse_error(
                token_info,
                format!("unexpected token: expected {}", stringify!($pat)),
            )),
            None => Err($self.add_end_error("unexpected end of input".to_owned())),
        }
    };
    ($self: expr, $v: expr, $pat: pat => $res: expr) => {
        match $v {
            Some(($pat, _)) => Ok($res),
            Some((_, token_info)) => Err($self.add_parse_error(
                token_info,
                format!("unexpected token: expected {}", stringify!($pat)),
            )),
            None => Err($self.add_end_error("unexpected end of input".to_owned())),
        }
    };
}

/// `match_expect_token!` の戻り値を無視するバージョン
///
/// # 注意
///
/// `unused_must_use` リントが experimental であるため、一時的にこのマクロで対処。
/// 将来的には `#[must_use]` を適切に使用する方向で検討。
macro_rules! match_expect_token_unused {
    ($self: expr, $v: expr, $pat: pat) => {
        let _ = match_expect_token!($self, $v, $pat);
    };
    ($self: expr, $v: expr, $pat: pat if $cond:expr) => {
        let _ = match_expect_token!($self, $v, $pat if $cond);
    };
}

// マクロを明示的にエクスポートする必要はない (mod.rs で #[macro_use] により自動的に利用可能になる)
