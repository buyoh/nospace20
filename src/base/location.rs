//! Source location information for AST nodes
//!
//! このモジュールは構文木のノードに位置情報を付与するための型を提供します。

/// ソースコード上の範囲
///
/// 位置はすべて元のソース文字列における文字（`char`）インデックスであり、バイトオフセットではない。
/// `&src[start..end]` のようなバイトでのスライスには使えない（マルチバイト文字で panic する）。
/// 行・列への変換は `TextCode::char_index_to_line` を使う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    /// 構文要素の先頭トークンの文字インデックス
    pub start: usize,
    /// 構文要素の直後のトークンの文字インデックス（含まない終端の目安。間の空白・コメントを含みうる）
    /// startと同じ値の場合もある
    ///
    /// 入力末尾で切れた式では現状 `start` より小さい値（0）になりうる
    /// （構文エラーとして報告され、後段には渡らない）。tree-parser-dedup Step 1.4 で修正されたらこの注記を消す。
    pub end: usize,
}

impl SourceLocation {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn from_single(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }
}
