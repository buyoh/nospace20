//! NospaceVM — 変数アクセス・スコープ管理・static 変数

use super::*;

impl NospaceVM {
    // ─── 変数アクセス ───

    pub(super) fn resolve_addr(&self, id: &IdentifierRef) -> i64 {
        if id.is_global {
            self.env.global_base_addr + id.local_index as i64
        } else {
            let depth = id.scope_depth;
            let idx = self.scope_stack.len().saturating_sub(1 + depth);
            self.scope_stack[idx] + id.local_index as i64
        }
    }

    pub(super) fn get_variable(&self, id: &IdentifierRef) -> i64 {
        self.env.allocator.get(self.resolve_addr(id))
    }

    pub(super) fn set_variable(&mut self, id: &IdentifierRef, v: i64) {
        let addr = self.resolve_addr(id);
        self.env.allocator.set(addr, v);
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

    // ─── static 変数 ───

    pub(super) fn save_static_vars(&mut self, func_idx: usize, scope_addr: i64) {
        if let Some(&static_addr) = self.env.function_static_addrs.get(&func_idx) {
            let vars: Vec<_> = self.scope.functions[func_idx]
                .block
                .scope
                .variables
                .iter()
                .filter(|v| v.is_static)
                .map(|v| (v.slot_index, v.array_size.unwrap_or(1)))
                .collect();
            for (slot, count) in vars {
                for i in 0..count {
                    let v = self.env.allocator.get(scope_addr + (slot + i) as i64);
                    self.env.allocator.set(static_addr + (slot + i) as i64, v);
                }
            }
        }
    }

    pub(super) fn load_static_vars(&mut self, func_idx: usize, scope_addr: i64) {
        if let Some(&static_addr) = self.env.function_static_addrs.get(&func_idx) {
            let vars: Vec<_> = self.scope.functions[func_idx]
                .block
                .scope
                .variables
                .iter()
                .filter(|v| v.is_static)
                .map(|v| (v.slot_index, v.array_size.unwrap_or(1)))
                .collect();
            for (slot, count) in vars {
                for i in 0..count {
                    let v = self.env.allocator.get(static_addr + (slot + i) as i64);
                    self.env.allocator.set(scope_addr + (slot + i) as i64, v);
                }
            }
        }
    }
}
