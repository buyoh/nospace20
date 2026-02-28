// 実行可能な中間表現の型定義

use crate::base::SourceLocation;
use crate::tree_parser::{Operator1, Operator2};

/// 式の値の型
///
/// コンパイラ内部で int と void の2種類の型を管理する。
/// 明示的な型定義構文は言語仕様に存在しないが、
/// コンパイル時に不正な型使用を検出するために使用する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// 整数型（i64）
    Int,
    /// 値なし型（while, else なし if, return なし関数など）
    Void,
}

impl ValueType {
    /// 2つの型をマージする（if/else の分岐統合用）
    /// 両方 Int のとき Int、それ以外は Void
    pub(crate) fn merge(self, other: ValueType) -> ValueType {
        match (self, other) {
            (ValueType::Int, ValueType::Int) => ValueType::Int,
            _ => ValueType::Void,
        }
    }
}

/// 条件式の評価モード
///
/// If/While の条件式がどのように true/false を判定するかを指定する。
/// 意味解析では常に NonZero が使用される。最適化パスが Zero/Negative に変換する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConditionMode {
    /// cond != 0 → true（既存動作、意味解析が生成）
    NonZero,
    /// cond == 0 → true（Whitespace: JumpIfZero を直接使用）
    Zero,
    /// cond < 0 → true（Whitespace: JumpIfNegative を直接使用）
    Negative,
}

/// 最適化パスで生成される内部組み込み関数の種類
///
/// 各バリアントは必要なデータを自身に保持する。
/// 意味解析では生成されず、最適化パスでのみ生成される。
pub(crate) enum InternalBuiltinFunctionKind {
    /// 標準入力から整数を読み、変数に直接格納（TEMP_PTR 経由を排除）
    Getiv(IdentifierRef),
    /// 標準入力から文字を読み、変数に直接格納
    Getcv(IdentifierRef),
}

/// 組み込み関数の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunctionKind {
    /// __puti(x) - 整数を10進数で出力
    Puti,
    /// __putc(x) - 文字を出力
    Putc,
    /// __geti() - 整数を入力
    Geti,
    /// __getc() - 文字を入力
    Getc,
    /// __clog(x) - デバッグログ出力
    Clog,
    /// __assert(x) - x が非ゼロであることをアサート
    Assert,
    /// __assert_not(x) - x がゼロであることをアサート
    AssertNot,
    /// __trace(x) - 実行回数をトレース
    Trace,
    /// __alloc(size) - メモリ確保 (--std-ext alloc)
    Alloc,
    /// __free(ptr) - メモリ解放 (--std-ext alloc)
    Free,
}

/// 解決済み識別子への参照
///
/// 変数・関数の識別子を文字列ではなく、
/// スコープ階層とローカルインデックスで管理することで、
/// 実行時の文字列検索を排除し、O(1) アクセスを実現する。
///
/// is_global フラグも保持。グローバル変数は Environment に保持されるため、
/// ローカル変数とは別の参照方法が必要。
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRef {
    /// スコープの深さ（0 = 現在のスコープ、1 = 親スコープ、...）
    pub scope_depth: usize,
    /// スコープ内でのインデックス
    pub local_index: usize,
    /// グローバル変数かどうか
    /// true の場合、Environment.global_variables でアクセス
    /// false の場合、LocalEnvironment.scope_stack でアクセス
    pub is_global: bool,
    /// static 変数を所有する関数のグローバルインデックス
    /// ネストされた関数から親関数の static 変数にアクセスする場合に使用。
    /// None の場合、現在の関数の static 変数として扱う。
    pub owning_func_index: Option<usize>,
}

/// 変数情報
///
/// is_static フラグを保持。static 変数は関数スコープ境界を越えてアクセス可能。
#[derive(Clone)]
pub(crate) struct Variable {
    // NOTE: ここに初期化情報は置かない
    /// スロットインデックス
    /// 変数が使用するメモリスロットの開始位置
    /// 配列の場合、開始位置のインデックス
    pub slot_index: usize,
    /// static フラグ
    /// true の場合、親の関数スコープからもアクセス可能
    /// グローバル変数は暗黙的に is_static = true
    pub is_static: bool,
    /// 配列サイズ。None なら通常変数（1スロット）、Some(n) なら n スロットの配列。
    pub array_size: Option<usize>,
    /// final フラグ。true の場合、初期値設定後は再代入不可。
    pub is_final: bool,
}

/// 位置情報付きの実行可能な式
pub(crate) struct LocatedExecExpression {
    pub expression: ExecExpression,
    pub location: SourceLocation,
}

/// 実行可能な式を表す。
///
/// `Expression` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない (パース成功後のみ生成される)
/// - スコープ解決済みの識別子情報を保持する
///   変数は IdentifierRef を使用することで、実行時の文字列検索を排除し、O(1) アクセスを実現。
///   関数も IdentifierRef を使用し、スコープ解決を行う（Phase 5 で実装）。
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<LocatedExecExpression>),
    Operation2(
        Operator2,
        Box<LocatedExecExpression>,
        Box<LocatedExecExpression>,
    ),
    /// if 式: (条件モード, 条件式, then ブロック, else ブロック)
    /// 意味解析では ConditionMode::NonZero で生成。最適化パスが Zero/Negative に変換可能。
    If(ConditionMode, Box<LocatedExecExpression>, Block, Block),
    Block(Block), // ブロックスコープ式
    /// 組み込み関数呼び出し
    /// Phase 6: 組み込み関数は BuiltinFunctionKind enum で識別
    BuiltinFunction(BuiltinFunctionKind, Vec<Box<LocatedExecExpression>>),
    /// ユーザー定義関数呼び出し
    /// Phase 5 で追加：スコープ解決済みの関数参照を保持
    UserFunction(IdentifierRef, Vec<Box<LocatedExecExpression>>),
    Factor(i64),
    /// 変数参照
    Variable(IdentifierRef),
    /// 配列アクセス: (変数参照, インデックス式, 配列サイズ)
    /// 配列サイズは境界チェックに使用
    ArrayAccess(IdentifierRef, Box<LocatedExecExpression>, usize),
    /// 最適化パスで生成される内部組み込み関数
    /// 意味解析では生成されず、最適化パスでのみ生成される。
    InternalBuiltinFunction(InternalBuiltinFunctionKind),
}

/// 実行可能な文を表す。
///
/// `Statement` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない
/// - 宣言文 (VariableDeclaration, FunctionDeclaration) を持たない
///   (宣言は `Scope` 構造に変換される)
pub(crate) enum ExecStatement {
    Return(Option<Box<LocatedExecExpression>>),
    Break,
    Continue,
    Expression(Box<LocatedExecExpression>),
    /// while 文: (条件モード, 条件式, ループ本体)
    /// 意味解析では ConditionMode::NonZero で生成。最適化パスが Zero/Negative に変換可能。
    While(ConditionMode, Box<LocatedExecExpression>, Block),
    /// for 文: (初期化ブロック, 条件モード, 条件ブロック, ステップブロック, 本体ブロック)
    /// repeat は tree_parser で For に脱糖される。
    /// continue は step ブロックへジャンプし、その後条件を再評価する。
    For(Block, ConditionMode, Block, Block, Block),
}

/// 位置情報を持つ実行可能な文
///
/// 意味解析フェーズで `LocatedStatement` の位置情報を引き継ぎ、
/// コンパイルエラー時に文レベルの位置情報を報告できるようにする。
pub(crate) struct LocatedExecStatement {
    pub statement: ExecStatement,
    pub location: SourceLocation,
}

/// ブロック（文の列とスコープ情報）
pub(crate) struct Block {
    pub scope: super::Scope,
    pub statements: Vec<LocatedExecStatement>,
}

impl LocatedExecExpression {
    /// 式の型を推論する
    ///
    /// `func_return_types` は全グローバル関数の戻り値型のスライス（インデックスが関数グローバルID）。
    pub(crate) fn infer_type(&self, func_return_types: &[ValueType]) -> ValueType {
        self.expression.infer_type(func_return_types)
    }
}

impl ExecExpression {
    /// 式の型を推論する
    ///
    /// `func_return_types` は全グローバル関数の戻り値型のスライス（インデックスが関数グローバルID）。
    pub(crate) fn infer_type(&self, func_return_types: &[ValueType]) -> ValueType {
        match self {
            ExecExpression::Factor(_) => ValueType::Int,
            ExecExpression::Variable(_) => ValueType::Int,
            ExecExpression::ArrayAccess(_, _, _) => ValueType::Int,
            ExecExpression::Operation1(_, _) => ValueType::Int,
            ExecExpression::Operation2(Operator2::Assign, _, rhs) => {
                // 代入式の型は右辺の型（型チェック済みなら常に Int）
                rhs.expression.infer_type(func_return_types)
            }
            ExecExpression::Operation2(_, _, _) => ValueType::Int,
            ExecExpression::If(_, _, then_block, else_block) => {
                // 両ブロックが Int のときのみ Int、それ以外は Void
                infer_block_type(then_block, func_return_types)
                    .merge(infer_block_type(else_block, func_return_types))
            }
            ExecExpression::Block(block) => infer_block_type(block, func_return_types),
            ExecExpression::BuiltinFunction(kind, _) => match kind {
                BuiltinFunctionKind::Trace => ValueType::Void,
                _ => ValueType::Int, // puti, putc, geti, getc, clog, assert, assert_not, alloc, free
            },
            ExecExpression::UserFunction(id_ref, _) => {
                // id_ref.local_index はグローバル関数インデックス
                func_return_types
                    .get(id_ref.local_index)
                    .copied()
                    .unwrap_or(ValueType::Void)
            }
            ExecExpression::InternalBuiltinFunction(kind) => match kind {
                InternalBuiltinFunctionKind::Getiv(_) => ValueType::Int,
                InternalBuiltinFunctionKind::Getcv(_) => ValueType::Int,
            },
        }
    }
}

/// ブロックの型を推論する（最後の式文の型）
pub(crate) fn infer_block_type(block: &Block, func_return_types: &[ValueType]) -> ValueType {
    match block.statements.last() {
        Some(located_stmt) => match &located_stmt.statement {
            ExecStatement::Expression(located_expr) => {
                located_expr.expression.infer_type(func_return_types)
            }
            _ => ValueType::Void, // 空ブロック、または最後が return/break/continue
        },
        None => ValueType::Void,
    }
}
