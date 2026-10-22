//! # Program
//!
//! 意味解析の結果（プログラム全体）。

use std::collections::BTreeMap;

use super::scope::{Function, Scope, SymbolTable};
use super::types::LocatedExecStatement;

/// 意味解析の結果（プログラム全体）
///
/// ルートスコープと、プログラム全体で 1 つだけある表（全関数・関数名・初期化文・グローバル領域の大きさ）を持つ。
/// 関数本体・ブロックのスコープ（`Scope`）はこれらを持たない。
pub struct Program {
    /// ルートスコープ（グローバル変数・構造体定義・ルートの識別子）
    pub root_scope: Scope,

    /// 全関数（ルート・名前空間・関数・ブロックで宣言したもの）。添字はグローバルな関数インデックス
    /// （WS コンパイラ・optimizer からアクセスするため pub(crate)）
    pub(crate) functions: Vec<Function>,

    /// デバッグ用シンボルテーブル
    pub symbol_table: SymbolTable,

    /// main 関数のインデックス（存在する場合）
    /// 関数名による検索を排除し、インデックスベースでアクセス
    pub main_function_index: Option<usize>,

    /// static 変数の初期化文
    /// ルート・関数・ブロックのすべての static 変数の初期化式を、ソース順に並べる
    /// （初期化子の中で宣言した static は、その static より先）。ルートの文（`root_statements`）より先に実行する
    pub(crate) static_init_statements: Vec<LocatedExecStatement>,

    /// ルートの実行文（非 static グローバル変数の初期化）
    pub(crate) root_statements: Vec<LocatedExecStatement>,

    /// グローバル領域のスロット数
    /// ルートの変数に加えて、関数スコープ・ブロックスコープのすべての static 変数の分
    /// （主に `root_scope.variables` に無い、名前の無いスロット）を含む
    pub(crate) global_slot_count: usize,
}

impl Program {
    /// 関数名（グローバルインデックス順）から `SymbolTable` と `__main` のインデックスを求めて組み立てる
    pub(super) fn new(
        root_scope: Scope,
        functions: Vec<Function>,
        function_names: Vec<String>,
        static_init_statements: Vec<LocatedExecStatement>,
        root_statements: Vec<LocatedExecStatement>,
        global_slot_count: usize,
    ) -> Self {
        let main_function_index = function_names.iter().position(|name| name == "__main");
        let function_name_to_index: BTreeMap<String, usize> = function_names
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.clone(), idx))
            .collect();
        Program {
            root_scope,
            functions,
            symbol_table: SymbolTable {
                function_names,
                function_name_to_index,
            },
            main_function_index,
            static_init_statements,
            root_statements,
            global_slot_count,
        }
    }

    #[cfg(test)]
    pub(crate) fn get_function(&self, id: &str) -> Option<&Function> {
        let idx = self.symbol_table.function_name_to_index.get(id)?;
        Some(&self.functions[*idx])
    }

    /// 指定した名前の関数が存在するかチェックする
    pub fn has_function(&self, id: &str) -> bool {
        self.symbol_table.function_name_to_index.contains_key(id)
    }

    /// ルートで実行する文（static の初期化文 → ルートの文。実行順）
    pub(crate) fn top_level_statements(&self) -> impl Iterator<Item = &LocatedExecStatement> {
        self.static_init_statements
            .iter()
            .chain(self.root_statements.iter())
    }

    /// `top_level_statements` の可変版
    pub(crate) fn top_level_statements_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut LocatedExecStatement> {
        self.static_init_statements
            .iter_mut()
            .chain(self.root_statements.iter_mut())
    }
}
