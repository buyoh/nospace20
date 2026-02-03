//! Source location information for AST nodes
//!
//! このモジュールは構文木のノードに位置情報を付与するための型を提供します。

/// ソースコード上の位置を表す構造体
#[derive(Clone, Debug)]
pub struct SourceLocation {
    /// 開始位置（バイト単位のインデックス）
    pub start: usize,
    /// 終了位置（バイト単位のインデックス）
    /// startと同じ値の場合もある
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
