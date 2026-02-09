//! スコープ管理とシンボル解決

use std::collections::BTreeMap;

use crate::{base::CodeParseError, code_parse_error};

use super::types::{Block, ExecStatement, IdentifierRef, Variable};

pub(super) struct IdentifierInfo {
    // name: String,
    pub idx: usize, // TODO: more safety
}

pub(super) enum Identifier {
    Function(IdentifierInfo),
    Variable(IdentifierInfo),
}

/// 関数情報
pub struct Function {
    pub args: Vec<String>, // TODO: change string to identifier_ptr
    /// 事前計算された引数のインデックス（Phase 2 最適化）
    /// 関数呼び出し時の引数初期化を O(args) にするため、
    /// 各引数の block.scope 内でのインデックスを保持
    pub arg_indices: Vec<usize>,
    pub block: Block,
    // pub identifier: String,
}

/// スコープ情報
///
/// Phase 2 で変数インデックス管理を追加。
/// 変数名からローカルインデックスへのマッピングを保持することで、
/// 実行時に Vec<i64> ベースの高速アクセスを可能にする。
///
/// Phase 3 で is_function_scope フラグを追加。関数スコープ境界を越える場合、
/// static 変数のみアクセス可能。
///
/// Phase 3 でルートスコープに実行文（グローバル変数の初期化）を追加。
pub struct Scope {
    pub(super) identifier_map: BTreeMap<String, Identifier>,

    /// 変数名からスロットインデックスへのマップ
    /// Phase 2 で追加: 識別子解決時に使用
    /// 配列の場合、配列の開始スロットインデックスを指す
    pub(crate) variable_indices: BTreeMap<String, usize>,

    /// 変数名から variables ベクタのインデックスへのマップ
    /// 配列対応のため追加: 配列情報を取得する際に使用
    pub(crate) variable_name_to_var_index: BTreeMap<String, usize>,

    pub(crate) variables: Vec<Variable>,

    /// 変数のスロット総数（配列サイズを考慮）
    /// Phase 2 で追加: インタプリタが Vec<i64> を初期化する際に使用
    pub(crate) variable_count: usize,

    pub(super) functions: Vec<Function>,

    /// Phase 3: このスコープが関数スコープかどうか
    /// true の場合、非 static 変数は親スコープからアクセス不可
    /// Root スコープと Function スコープで true
    pub(crate) is_function_scope: bool,

    /// Phase 3: ルートスコープの実行文（グローバル変数の初期化）
    /// 関数スコープ・ブロックスコープでは空
    pub(crate) root_statements: Vec<ExecStatement>,
}

impl Scope {
    pub(crate) fn get_function(&self, id: &str) -> Option<&Function> {
        if let Some(Identifier::Function(info)) = self.identifier_map.get(id) {
            Some(&self.functions[info.idx])
        } else {
            None
        }
    }

    pub(crate) fn get_variable(&self, id: &str) -> Option<&Variable> {
        if let Some(Identifier::Variable(info)) = self.identifier_map.get(id) {
            Some(&self.variables[info.idx])
        } else {
            None
        }
    }

    /// 指定した名前の関数が存在するかチェックする
    pub fn has_function(&self, id: &str) -> bool {
        self.get_function(id).is_some()
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
/// Phase 3 で追加。関数境界チェックのため、各スコープの追加情報を保持する。
#[derive(Clone)]
pub(super) struct ScopeInfo<'a> {
    /// 変数名からスロットインデックスへのマップ
    pub var_indices: &'a BTreeMap<String, usize>,
    /// 変数名から variables ベクタのインデックスへのマップ
    pub var_name_to_var_index: &'a BTreeMap<String, usize>,
    /// 変数情報（static フラグ、配列サイズ確認用）
    pub variables: &'a Vec<Variable>,
    /// このスコープが関数スコープかどうか
    pub is_function_scope: bool,
}

/// スコープ解決のためのコンテキスト
///
/// Phase 2 で導入。2パス解析のパス2で使用され、
/// 変数名・関数名を IdentifierRef に解決する。
///
/// Phase 3 で関数境界チェックを追加。親の関数スコープの非 static 変数には
/// アクセスできないようにする。
pub(super) struct ScopeResolver<'a> {
    /// スコープスタック（末尾が現在のスコープ）
    /// Phase 3: スコープ情報を保持するように変更
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
        is_function_scope: bool,
    ) {
        self.scope_stack.push(ScopeInfo {
            var_indices,
            var_name_to_var_index,
            variables,
            is_function_scope,
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

                return Some(IdentifierRef {
                    scope_depth: depth,
                    local_index,
                    is_global,
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
}

/// スコープビルダー
///
/// スコープ構築時に使用する内部構造
pub(super) struct ScopeBuilder {
    pub identifier_map: BTreeMap<String, Identifier>,
    pub variables: Vec<Variable>,
    pub functions: Vec<Function>,
}

impl ScopeBuilder {
    pub fn new() -> Self {
        Self {
            identifier_map: BTreeMap::new(),
            variables: vec![],
            functions: vec![],
        }
    }

    pub fn build(self, is_function_scope: bool, root_statements: Vec<ExecStatement>) -> Scope {
        // 変数名からスロットインデックスへのマッピングを構築
        // 配列の場合、変数の開始スロットインデックスを記録
        let mut variable_indices = BTreeMap::new();
        let mut variable_name_to_var_index = BTreeMap::new();
        let mut slot_index = 0;
        for (var_idx, var) in self.variables.iter().enumerate() {
            variable_indices.insert(var.identifier.clone(), slot_index);
            variable_name_to_var_index.insert(var.identifier.clone(), var_idx);
            slot_index += var.array_size.unwrap_or(1);
        }
        let variable_count = slot_index;

        Scope {
            identifier_map: self.identifier_map,
            variable_indices,
            variable_name_to_var_index,
            variables: self.variables,
            variable_count,
            functions: self.functions,
            is_function_scope,
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
        self.add_identifier(name, Identifier::Variable(IdentifierInfo { idx: vi }))
    }

    pub fn add_function(&mut self, name: &str, func: Function) -> Result<(), Vec<CodeParseError>> {
        let fi = self.functions.len();
        self.functions.push(func);
        self.add_identifier(name, Identifier::Function(IdentifierInfo { idx: fi }))
    }
}
