//! # ExecContext
//!
//! 解析中のコードが「どの実行単位の中にあるか」を表す実行文脈。
//! スコープ（名前の可視範囲）とは別の軸で、`return` / `break` / `continue` の可否と、
//! static 変数の初期化子から参照できる変数の範囲を判定するのに使う。

/// 解析中のコードが属する実行単位
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExecUnit {
    /// ルートの文（グローバル変数の初期化）
    RootInit,
    /// 関数本体。`return` の戻り値の型の検査に使う
    FunctionBody { func_index: usize },
    /// static 変数の初期化子（`__main` の前に 1 回だけ実行される）
    StaticInitializer,
}

/// 実行文脈
///
/// 名前の可視範囲（スコープスタック）とは独立に、「どの実行単位の中か」と
/// 「その中でループの内側か」を表す。関数宣言と static の初期化子で新しい実行単位に入り、
/// ブロック（`{}`・`if`・ブロック式・ブロックエイリアスの展開）はそのまま引き継ぐ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExecContext {
    pub unit: ExecUnit,
    /// 実行単位に入ったときのスコープスタックの長さ。
    /// スタック上の位置がこれより小さい段は、実行単位の外
    pub unit_base: usize,
    /// 実行単位の中で、今いる位置を囲むループの数（実行単位に入ると 0 に戻る）
    pub loop_depth: usize,
}

impl ExecContext {
    pub(super) fn root() -> Self {
        Self {
            unit: ExecUnit::RootInit,
            unit_base: 0,
            loop_depth: 0,
        }
    }

    /// 関数本体に入る。`scope_stack_len` は関数スコープを積む前の長さ
    pub(super) fn function_body(func_index: usize, scope_stack_len: usize) -> Self {
        Self {
            unit: ExecUnit::FunctionBody { func_index },
            unit_base: scope_stack_len,
            loop_depth: 0,
        }
    }

    /// static の初期化子に入る。`scope_stack_len` は宣言したスコープまで積んだ長さ
    /// （宣言したスコープ自身も実行単位の外になる）
    pub(super) fn static_initializer(scope_stack_len: usize) -> Self {
        Self {
            unit: ExecUnit::StaticInitializer,
            unit_base: scope_stack_len,
            loop_depth: 0,
        }
    }

    /// ループの条件・本体に入る
    pub(super) fn in_loop(self) -> Self {
        Self {
            loop_depth: self.loop_depth + 1,
            ..self
        }
    }
}
