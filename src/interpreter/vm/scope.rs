//! NospaceVM — 変数アクセス・スコープ管理・static 変数

use super::*;

impl NospaceVM {
    // ─── 変数アクセス ───

    /// static 変数は static 領域のアドレスへ解決する（`FunctionStaticStorage::resolve`）
    pub(super) fn resolve_addr(&self, id: &IdentifierRef) -> i64 {
        if id.is_global {
            self.env.global_base_addr + id.local_index as i64
        } else if let Some(addr) = self.env.function_statics.resolve(
            id,
            self.func_frames.last().copied(),
            self.scope_stack.len(),
        ) {
            addr
        } else {
            let depth = id.scope_depth;
            let idx = self.scope_stack.len().saturating_sub(1 + depth);
            self.scope_stack[idx] + id.local_index as i64
        }
    }

    pub(super) fn get_variable(&self, id: &IdentifierRef) -> Result<i64, InterpretError> {
        self.env.allocator.get(self.resolve_addr(id))
    }

    pub(super) fn set_variable(
        &mut self,
        id: &IdentifierRef,
        v: i64,
    ) -> Result<(), InterpretError> {
        let addr = self.resolve_addr(id);
        self.env.allocator.set(addr, v)
    }

    pub(super) fn enter_block(&mut self, scope: &crate::semantic_analyzer::Scope) -> i64 {
        let base = self
            .env
            .allocator
            .alloc_internal_uninit(scope.variable_count, self.env.config.randomize_uninit);
        self.scope_stack.push(base);
        base
    }

    pub(super) fn leave_scope(&mut self, scope_addr: i64) {
        if self.scope_stack.last() == Some(&scope_addr) {
            self.scope_stack.pop();
        }
        self.env.allocator.free_internal(scope_addr);
    }

    /// 関数から出る: 関数スコープと、関数内で残っているブロックの領域を `scope_stack` から外して解放する
    ///
    /// static 変数の解決（`FunctionStaticStorage::resolve`）は「関数スコープが
    /// `scope_stack[FuncFrame::scope_index]` にある」ことを前提とするため、関数の出口で
    /// 呼び出し時に記録した `scope_index` まで `scope_stack` を戻してこれを保証する。
    /// 通常は関数スコープだけが残っている（for ループの init ブロックは `propagate_flow` で外す）。
    pub(super) fn leave_func_scope(&mut self, scope_addr: i64) {
        let frame = self
            .func_frames
            .pop()
            .expect("internal error: no function frame to leave");
        debug_assert_eq!(self.scope_stack.get(frame.scope_index), Some(&scope_addr));
        while self.scope_stack.len() > frame.scope_index {
            let addr = self.scope_stack.pop().unwrap();
            self.env.allocator.free_internal(addr);
        }
    }
}
