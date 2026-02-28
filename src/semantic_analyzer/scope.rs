//! スコープ管理とシンボル解決

use std::collections::BTreeMap;

use crate::{base::CodeParseError, code_parse_error};

use crate::tree_parser::LocatedStatement;
use super::types::{Block, IdentifierRef, LocatedExecStatement, ValueType, Variable};

#[derive(Clone, Copy)]
pub(super) struct FunctionIndex(pub usize, pub usize, pub ValueType); // (global_index, arg_count, return_type)

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(super) struct VariableIndex(pub usize);

#[derive(Clone)]
pub(super) enum Identifier {
    Function(FunctionIndex),
    #[allow(dead_code)]
    Variable(VariableIndex),
}

/// デバッグ用シンボルテーブル
///
/// インデックスから識別子名への逆引きを提供する。
/// ランタイム動作には不要だが、デバッグ出力・エラーメッセージ・
/// コンパイラのラベル生成で使用される。
pub struct SymbolTable {
    /// 関数インデックス → 関数名
    pub function_names: Vec<String>,
    /// 関数名 → 関数インデックス（逆引き）
    pub function_name_to_index: BTreeMap<String, usize>,
}

/// 関数情報
pub struct Function {
    /// 事前計算された引数のインデックス（最適化）
    /// 関数呼び出し時の引数初期化を O(args) にするため、
    /// 各引数の block.scope 内でのインデックスを保持
    pub arg_indices: Vec<usize>,
    pub block: Block,
    /// 戻り値型（内部型システム）
    /// 関数本体に return: expr; が存在する → Int
    /// return: が存在しない（暗黙の void return）→ Void
    pub return_type: ValueType,
    /// 最適化パス（dead_code）によって未使用（到達不可能）とマークされたかどうか
    /// true の場合、コード生成・実行でスキップされる
    pub is_unused: bool,
    // pub identifier: String,
}

impl Function {
    /// dead_code パスが生成するダミー関数
    ///
    /// 空のブロック・Void 戻り値型を持つ最小限の関数。
    /// コード生成時にスキップされる。
    pub fn dummy() -> Self {
        use crate::semantic_analyzer::types::Block;
        let dummy_scope = Scope {
            identifier_map: BTreeMap::new(),
            variable_indices: BTreeMap::new(),
            variable_name_to_var_index: BTreeMap::new(),
            variables: Vec::new(),
            variable_count: 0,
            functions: Vec::new(),
            symbol_table: SymbolTable {
                function_names: Vec::new(),
                function_name_to_index: BTreeMap::new(),
            },
            main_function_index: None,
            static_init_statements: Vec::new(),
            root_statements: Vec::new(),
        };
        Function {
            arg_indices: Vec::new(),
            block: Block {
                scope: dummy_scope,
                statements: Vec::new(),
            },
            return_type: ValueType::Void,
            is_unused: true,
        }
    }

    /// この関数が未使用（到達不可能）かどうかを返す
    pub fn is_unused(&self) -> bool {
        self.is_unused
    }
}

/// スコープ情報
///
/// 変数インデックス管理を含む。
/// 変数名からローカルインデックスへのマッピングを保持することで、
/// 実行時に Vec<i64> ベースの高速アクセスを可能にする。
///
/// is_function_scope フラグも保持。関数スコープ境界を越える場合、
/// static 変数のみアクセス可能。
///
/// ルートスコープには実行文（グローバル変数の初期化）も追加。
pub struct Scope {
    pub(super) identifier_map: BTreeMap<String, Identifier>,

    /// 変数名からスロットインデックスへのマップ
    /// 識別子解決時に使用
    /// 配列の場合、配列の開始スロットインデックスを指す
    pub(crate) variable_indices: BTreeMap<String, usize>,

    /// 変数名から variables ベクタのインデックスへのマップ
    /// 配列対応のため追加: 配列情報を取得する際に使用
    pub(crate) variable_name_to_var_index: BTreeMap<String, usize>,

    pub(crate) variables: Vec<Variable>,

    /// 変数のスロット総数（配列サイズを考慮）
    /// インタプリタが Vec<i64> を初期化する際に使用
    pub(crate) variable_count: usize,

    /// Phase 5: 関数リストを pub(crate) に変更（interpreter からアクセスするため）
    pub(crate) functions: Vec<Function>,

    /// デバッグ用シンボルテーブル
    pub symbol_table: SymbolTable,

    /// main 関数のインデックス（存在する場合）
    /// Phase 6: 関数名による検索を排除し、インデックスベースでアクセス
    pub main_function_index: Option<usize>,

    /// static 変数の初期化文
    /// 関数スコープの場合: 関数内の static 変数の初期化式
    /// ルートスコープの場合: ルートレベルの static 変数の初期化式（非 static より先に実行）
    pub(crate) static_init_statements: Vec<LocatedExecStatement>,

    /// ルートスコープの実行文（非 static グローバル変数の初期化）
    /// 関数スコープ・ブロックスコープでは空
    pub(crate) root_statements: Vec<LocatedExecStatement>,
}

impl Scope {
    pub(crate) fn get_function(&self, id: &str) -> Option<&Function> {
        let idx = self.symbol_table.function_name_to_index.get(id)?;
        Some(&self.functions[*idx])
    }

    /// 指定した名前の関数が存在するかチェックする
    pub fn has_function(&self, id: &str) -> bool {
        self.symbol_table.function_name_to_index.contains_key(id)
    }
}

/// スコープの種類
pub(super) enum ScopeType {
    Root,
    Function,
    Block,
}

/// スコープ情報（ScopeResolver 用）
///
/// 関数境界チェックのため、各スコープの追加情報を保持する。
/// Phase 5: 関数の可視性チェックのため、関数マップも保持。
#[derive(Clone)]
pub(super) struct ScopeInfo<'a> {
    /// 変数名からスロットインデックスへのマップ
    pub var_indices: &'a BTreeMap<String, usize>,
    /// 変数名から variables ベクタのインデックスへのマップ
    pub var_name_to_var_index: &'a BTreeMap<String, usize>,
    /// 変数情報（static フラグ、配列サイズ確認用）
    pub variables: &'a Vec<Variable>,
    /// 関数名からマップへの参照（関数可視性チェック用）
    /// Phase 5 で追加
    pub func_map: &'a BTreeMap<String, Identifier>,
    /// constexpr 定数テーブル（名前 → 定数値）
    pub constexpr_table: &'a BTreeMap<String, i64>,
    /// alias テーブル（名前 → ターゲット識別子名）
    pub alias_map: &'a BTreeMap<String, String>,
    /// ブロックエイリアステーブル（名前 → AST 本体）
    pub block_alias_map: &'a BTreeMap<String, Vec<LocatedStatement>>,
    /// このスコープが関数スコープかどうか
    pub is_function_scope: bool,
    /// この関数スコープのグローバル関数インデックス
    /// ネストされた関数から親の static 変数にアクセスする際に使用
    pub func_global_index: Option<usize>,
}

/// スコープ解決のためのコンテキスト
///
/// 2パス解析のパス2で使用され、
/// 変数名・関数名を IdentifierRef に解決する。
///
/// 関数境界チェックも実装。親の関数スコープの非 static 変数には
/// アクセスできないようにする。
pub(super) struct ScopeResolver<'a> {
    /// スコープスタック（末尾が現在のスコープ）
    pub scope_stack: Vec<ScopeInfo<'a>>,
}

impl<'a> ScopeResolver<'a> {
    pub fn new() -> Self {
        Self {
            scope_stack: Vec::new(),
        }
    }

    pub fn enter_scope(
        &mut self,
        var_indices: &'a BTreeMap<String, usize>,
        var_name_to_var_index: &'a BTreeMap<String, usize>,
        variables: &'a Vec<Variable>,
        func_map: &'a BTreeMap<String, Identifier>,
        constexpr_table: &'a BTreeMap<String, i64>,
        alias_map: &'a BTreeMap<String, String>,
        block_alias_map: &'a BTreeMap<String, Vec<LocatedStatement>>,
        is_function_scope: bool,
        func_global_index: Option<usize>,
    ) {
        self.scope_stack.push(ScopeInfo {
            var_indices,
            var_name_to_var_index,
            variables,
            func_map,
            constexpr_table,
            alias_map,
            block_alias_map,
            is_function_scope,
            func_global_index,
        });
    }

    pub fn leave_scope(&mut self) {
        self.scope_stack.pop();
    }

    /// 変数名を解決し、IdentifierRef を返す
    ///
    /// スコープスタックを逆順に探索し、最も近いスコープの変数を見つける。
    /// 関数スコープ境界を越えた場合、static 変数のみアクセス可能。
    /// 見つからない場合は None を返す。
    pub fn resolve_variable(&self, name: &str) -> Option<IdentifierRef> {
        // 最初に見つけた関数スコープ（自分の関数）より外側の関数スコープを越えた場合、境界を越えたとする
        let mut first_function_scope_depth: Option<usize> = None;

        for (depth, scope_info) in self.scope_stack.iter().rev().enumerate() {
            // 最初の関数スコープを記録
            if scope_info.is_function_scope && first_function_scope_depth.is_none() {
                first_function_scope_depth = Some(depth);
            }

            if let Some(&local_index) = scope_info.var_indices.get(name) {
                // 変数情報を取得（var_name_to_var_index 経由）
                let var_idx = scope_info.var_name_to_var_index.get(name)?;
                let var = &scope_info.variables[*var_idx];

                // 関数境界を越えたかチェック
                // first_function_scope_depth より外側（depth が大きい）の関数スコープに変数がある場合
                let crossed_function_boundary =
                    if let Some(first_func_depth) = first_function_scope_depth {
                        depth > first_func_depth && scope_info.is_function_scope
                    } else {
                        // まだ関数スコープに入っていない（グローバルスコープのみ探索中）
                        false
                    };

                // 関数境界を越えた場合、static 変数のみアクセス可能
                if crossed_function_boundary && !var.is_static {
                    // 非 static 変数はスキップして探索継続
                    continue;
                }

                // グローバル変数かどうかを判定
                // スタックの最下層（depth == scope_stack.len() - 1）がルートスコープ
                let is_global = depth == self.scope_stack.len() - 1
                    && self
                        .scope_stack
                        .first()
                        .map(|s| s.is_function_scope)
                        .unwrap_or(false);

                // 関数境界を越えた static 変数アクセスの場合、
                // 変数を所有する関数のグローバルインデックスを記録
                let owning_func_index = if crossed_function_boundary && var.is_static {
                    scope_info.func_global_index
                } else {
                    None
                };

                return Some(IdentifierRef {
                    scope_depth: depth,
                    local_index,
                    is_global,
                    owning_func_index,
                });
            }
        }
        None
    }

    /// 変数の配列サイズを取得
    ///
    /// None の場合、変数が見つからない
    /// Some(None) の場合、通常変数（配列ではない）
    /// Some(Some(n)) の場合、サイズ n の配列
    pub fn get_array_size(&self, name: &str) -> Option<Option<usize>> {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(&var_idx) = scope_info.var_name_to_var_index.get(name) {
                return Some(scope_info.variables[var_idx].array_size);
            }
        }
        None
    }

    /// 関数名を解決し、IdentifierRef を返す
    ///
    /// Phase 5 で追加：ネスト関数の可視性チェック
    /// Phase 5 修正：全関数はグローバルに格納されるため、常に is_global=true を返す
    ///
    /// スコープスタックを逆順に探索し、最も近いスコープの関数を見つける。
    /// 子スコープの関数は見えないため、探索は現在のスコープから親に向かってのみ行う。
    /// 見つからない場合は None を返す。
    ///
    /// local_index はグローバル関数リストのインデックスを指す。
    pub fn resolve_function(&self, name: &str) -> Option<IdentifierRef> {
        for (_depth, scope_info) in self.scope_stack.iter().rev().enumerate() {
            if let Some(Identifier::Function(info)) = scope_info.func_map.get(name) {
                // Phase 5: 全関数はルートスコープにフラット化されているため、
                // 常に is_global=true、scope_depth=0 を返す
                // local_index はグローバルインデックス
                return Some(IdentifierRef {
                    scope_depth: 0,
                    local_index: info.0,
                    is_global: true,
                    owning_func_index: None,
                });
            }
        }
        None
    }

    /// 関数の期待される引数数を取得する
    pub fn get_function_arg_count(&self, name: &str) -> Option<usize> {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(Identifier::Function(info)) = scope_info.func_map.get(name) {
                return Some(info.1);
            }
        }
        None
    }

    /// constexpr 定数名を解決し、定数値を返す
    ///
    /// スコープスタックを内側から外側へ探索し、最も近いスコープの constexpr 値を返す。
    /// 見つからない場合は None を返す。
    pub fn resolve_constexpr(&self, name: &str) -> Option<i64> {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(&v) = scope_info.constexpr_table.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// ブロックエイリアス名を解決し、AST 本体を返す
    ///
    /// スコープスタックを内側から外側へ探索する。
    /// 見つからない場合は None を返す。
    pub fn resolve_block_alias(&self, name: &str) -> Option<&Vec<LocatedStatement>> {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(body) = scope_info.block_alias_map.get(name) {
                return Some(body);
            }
        }
        None
    }

    /// エイリアス名をチェーン解決して最終的な識別子名を返す
    ///
    /// スコープスタックを内側から外側へ探索し、エイリアスチェーンを解決する。
    /// 巡回参照が検出された場合はエラーを返す。
    /// エイリアスが定義されていない場合は、元の名前をそのまま返す。
    pub fn resolve_alias_chain(&self, name: &str) -> Result<String, String> {
        use std::collections::BTreeSet;
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut current = name.to_string();
        loop {
            if visited.contains(&current) {
                return Err(format!(
                    "circular alias reference detected: '{}' is part of a cyclic definition",
                    name
                ));
            }
            visited.insert(current.clone());
            // スコープスタックを内側から探索
            let mut found = None;
            for scope_info in self.scope_stack.iter().rev() {
                if let Some(target) = scope_info.alias_map.get(&current) {
                    found = Some(target.clone());
                    break;
                }
            }
            match found {
                Some(next) => current = next,
                None => return Ok(current),
            }
        }
    }
}

/// スコープビルダー
///
/// スコープ構築時に使用する内部構造
/// Phase 5: functions と function_names を削除（グローバル管理に移行）
pub(super) struct ScopeBuilder {
    pub identifier_map: BTreeMap<String, Identifier>,
    pub variables: Vec<Variable>,
    /// 変数名のリスト（variables と同じ順序）
    pub variable_names: Vec<String>,
    /// static 変数の初期化文を一時的に保持
    pub static_init_statements: Vec<LocatedExecStatement>,
}

impl ScopeBuilder {
    pub fn new() -> Self {
        Self {
            identifier_map: BTreeMap::new(),
            variables: vec![],
            variable_names: vec![],
            static_init_statements: vec![],
        }
    }

    /// スコープをビルドする
    /// Phase 5: functions と function_names を引数として受け取る
    /// ルートスコープの場合のみ有効な値を渡し、それ以外は空の Vec を渡す
    pub fn build(
        mut self,
        root_statements: Vec<LocatedExecStatement>,
        functions: Vec<Function>,
        function_names: Vec<String>,
    ) -> Scope {
        // 変数名からスロットインデックスへのマッピングを構築
        // 配列の場合、変数の開始スロットインデックスを記録
        // 同時に各 Variable に slot_index を設定
        let mut variable_indices = BTreeMap::new();
        let mut variable_name_to_var_index = BTreeMap::new();
        let mut slot_index = 0;
        for (var_idx, var) in self.variables.iter_mut().enumerate() {
            var.slot_index = slot_index;
            let var_name = &self.variable_names[var_idx];
            variable_indices.insert(var_name.clone(), slot_index);
            variable_name_to_var_index.insert(var_name.clone(), var_idx);
            slot_index += var.array_size.unwrap_or(1);
        }
        let variable_count = slot_index;

        // Phase 6: __main 関数のインデックスを解決
        let main_function_index = function_names.iter().position(|name| name == "__main");

        // Phase 6: SymbolTable を構築
        let mut function_name_to_index = BTreeMap::new();
        for (idx, name) in function_names.iter().enumerate() {
            function_name_to_index.insert(name.clone(), idx);
        }
        let symbol_table = SymbolTable {
            function_names,
            function_name_to_index,
        };

        Scope {
            identifier_map: self.identifier_map,
            variable_indices,
            variable_name_to_var_index,
            variables: self.variables,
            variable_count,
            functions,
            symbol_table,
            main_function_index,
            static_init_statements: self.static_init_statements,
            root_statements,
        }
    }

    pub fn add_identifier(
        &mut self,
        name: &str,
        identifier: Identifier,
    ) -> Result<(), Vec<CodeParseError>> {
        if self.identifier_map.contains_key(name) {
            return Err(vec![code_parse_error!(format!(
                "semantic error: the name '{}' is already used",
                name
            ))]);
        }
        self.identifier_map.insert(name.to_string(), identifier);
        Ok(())
    }

    pub fn add_variable(&mut self, name: &str, var: Variable) -> Result<(), Vec<CodeParseError>> {
        let vi = self.variables.len();
        self.variables.push(var);
        self.variable_names.push(name.to_string());
        self.add_identifier(name, Identifier::Variable(VariableIndex(vi)))
    }
}
