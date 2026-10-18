//! Whitespace ラベルの写像表

use std::collections::HashMap;

use crate::base::ws_types::{LabelId, WsChar};

/// Whitespace のラベル（Space/Tab のビット列）を連番の LabelId に写像する表
///
/// Whitespace のラベルは符号を持たない任意長のビット列であり、数値として解釈すると
/// `[S,T]` と `[T,T]` のような別ラベルが衝突する。ビット列そのものを key にして区別する。
pub(super) struct LabelInterner<'a> {
    /// ビット列（終端の LF を含まない）→ 割り当て済み ID
    ids: HashMap<&'a [WsChar], LabelId>,
}

impl<'a> LabelInterner<'a> {
    pub(super) fn new() -> Self {
        Self {
            ids: HashMap::new(),
        }
    }

    /// 既知のビット列なら既存 ID、未知なら次の連番を割り当てて返す。
    /// ID が u32 に収まらない場合（ラベル数 > u32::MAX）は None
    pub(super) fn intern(&mut self, bits: &'a [WsChar]) -> Option<LabelId> {
        if let Some(&id) = self.ids.get(bits) {
            return Some(id);
        }
        let id = LabelId(u32::try_from(self.ids.len()).ok()?);
        self.ids.insert(bits, id);
        Some(id)
    }
}
