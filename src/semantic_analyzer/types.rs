// 実行可能な中間表現の型定義

use crate::tree_parser::{Operator1, Operator2};

/// 解決済み識別子への参照
///
/// Phase 2 で導入。変数・関数の識別子を文字列ではなく、
/// スコープ階層とローカルインデックスで管理することで、
/// 実行時の文字列検索を排除し、O(1) アクセスを実現する。
///
/// Phase 3 で is_global フラグを追加。グローバル変数は Environment に保持されるため、
/// ローカル変数とは別の参照方法が必要。
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRef {
    /// スコープの深さ（0 = 現在のスコープ、1 = 親スコープ、...）
    pub scope_depth: usize,
    /// スコープ内でのインデックス
    pub local_index: usize,
    /// Phase 3: グローバル変数かどうか
    /// true の場合、Environment.global_variables でアクセス
    /// false の場合、LocalEnvironment.scope_stack でアクセス
    pub is_global: bool,
}

/// 変数情報
///
/// Phase 3 で is_static フラグを追加。static 変数は関数スコープ境界を越えてアクセス可能。
#[derive(Clone)]
pub(crate) struct Variable {
    // NOTE: ここに初期化情報は置かない
    pub identifier: String, // TODO: use IdentifierInfo
    /// Phase 3: static フラグ
    /// true の場合、親の関数スコープからもアクセス可能
    /// グローバル変数は暗黙的に is_static = true
    pub is_static: bool,
    /// 配列サイズ。None なら通常変数（1スロット）、Some(n) なら n スロットの配列。
    pub array_size: Option<usize>,
}

/// 実行可能な式を表す。
///
/// `Expression` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない (パース成功後のみ生成される)
/// - スコープ解決済みの識別子情報を保持する (Phase 2 で実装)
///   変数は IdentifierRef を使用することで、実行時の文字列検索を排除し、O(1) アクセスを実現。
///   関数は現状組み込み関数のみのため、文字列のまま保持。
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    /// 関数呼び出し
    /// Phase 2: 組み込み関数のみのため String のまま（ユーザー定義関数は Phase 3 以降）
    Function(String, Vec<Box<ExecExpression>>),
    Factor(i64),
    /// 変数参照
    /// Phase 2: String から IdentifierRef に変更
    Variable(IdentifierRef),
    /// 配列アクセス: (変数参照, インデックス式, 配列サイズ)
    /// 配列サイズは境界チェックに使用
    ArrayAccess(IdentifierRef, Box<ExecExpression>, usize),
}

/// 実行可能な文を表す。
///
/// `Statement` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない
/// - 宣言文 (VariableDeclaration, FunctionDeclaration) を持たない
///   (宣言は `Scope` 構造に変換される)
pub(crate) enum ExecStatement {
    Return(Box<ExecExpression>),
    Break,
    Continue,
    Expression(Box<ExecExpression>),
}

/// ブロック（文の列とスコープ情報）
pub(crate) struct Block {
    pub scope: super::Scope,
    pub statements: Vec<ExecStatement>,
}
