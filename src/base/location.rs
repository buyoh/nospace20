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
    /// 常に `start` 以上。入力末尾（直後のトークンが無い）では `start` と同じ値の場合もある
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
