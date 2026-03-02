# 詳細設計

## 公開インターフェース（WhitespaceVM 準拠）

`NospaceVM` は `WhitespaceVM` と可能な限り同じパターンのインターフェースを持つ。

### StepResult

nospace 用の `StepResult` を独自定義する。Whitespace の `RuntimeError` とは異なるエラー型 (`InterpretError`) を使用するため。

```rust
// src/interpreter/vm.rs (新規)

/// nospace インタプリタの実行結果
#[derive(Debug)]
pub enum StepResult {
    /// 実行継続中（バジェット消費で中断）
    Suspended,
    /// 正常終了
    Complete {
        return_value: Option<i64>,
    },
    /// 実行時エラー
    Error(InterpretError),
}
```

### NospaceVM 構造体

```rust
// src/interpreter/vm.rs (新規)

/// nospace ステップ実行 VM
///
/// 明示的スタックマシンとして全実行状態を保持する。
/// `step()` / `run()` で指定ステップずつ実行し、任意のタイミングで中断・再開可能。
///
/// ## WhitespaceVM との対応
///
/// | WhitespaceVM | NospaceVM |
/// |---|---|
/// | `from_source(ws)` | `from_source(nospace)` |
/// | `step(budget)` | `step(budget)` |
/// | `run(max_steps)` | `run(max_steps)` |
/// | `is_complete()` | `is_complete()` |
/// | `total_steps()` | `total_steps()` |
/// | `get_stdout_string()` | `get_stdout_string()` |
/// | `with_stdin(buf)` | `with_stdin(buf)` |
/// | `with_io(stdin, stdout)` | `with_io(stdin, stdout)` |
/// | `with_interactive_stdin()` | (将来拡張) |
pub struct NospaceVM {
    // === プログラム ===
    /// 解析済みスコープ（AST を所有）
    scope: Scope,

    // === 実行状態 ===
    /// フレームスタック（明示的な実行位置管理）
    frames: Vec<Frame>,
    /// 値スタック（式評価の中間値・戻り値を格納）
    value_stack: Vec<i64>,
    /// フロー制御 (Return/Break/Continue の伝播用)
    flow: Option<FlowControl>,
    /// スコープスタック: 各スコープのアロケータベースアドレス
    scope_stack: Vec<i64>,

    // === I/O・メモリ ===
    /// 実行環境（stdin, stdout, アロケータ, メトリクス等）
    env: Environment,
    /// テスト用: stdout の内容を型安全に取得するための共有バッファ
    stdout_capture: Option<Rc<RefCell<Vec<u8>>>>,

    // === メトリクス ===
    /// 総式評価回数
    total_steps: usize,

    // === 拡張 ===
    /// トレース出力（__trace 組み込み関数の結果）
    pub traced: BTreeMap<i64, i64>,

    // === 状態フラグ ===
    /// 実行完了済みかどうか
    completed: bool,
    /// 戻り値（main 関数の return 値）
    return_value: Option<i64>,
}
```

### コンストラクタ（Builder パターン）

`WhitespaceVM` と同様の Builder パターンを採用:

```rust
impl NospaceVM {
    /// nospace ソースコードから VM を構築
    ///
    /// パース → 意味解析 → VM 構築を一括実行する。
    pub fn from_source(source: &str) -> Result<Self, NospaceError> {
        let tokens = parse_to_tokens(&source.to_string())?;
        let tree = parse_to_tree(&tokens)?;
        let scope = semantic_analyze(&tree)?;
        Self::from_scope(scope)
    }

    /// 解析済み Scope から VM を構築
    pub fn from_scope(scope: Scope) -> Result<Self, InterpretError> {
        // Scope を所有し、初期フレームを積む
        Ok(Self { ... })
    }

    /// stdin を設定する（stdout はデフォルトの capture を維持）
    pub fn with_stdin(mut self, stdin: Box<dyn BufRead>) -> Self { ... }

    /// I/O バッファを指定して構築
    pub fn with_io(mut self, stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self { ... }

    /// EnvironmentConfig を設定
    pub fn with_config(mut self, config: EnvironmentConfig) -> Self { ... }
}
```

### 実行メソッド

```rust
impl NospaceVM {
    /// 指定ステップ数だけ実行し、結果を返す
    ///
    /// budget 回の式評価を実行。途中で完了/エラーに到達した場合は即座に返す。
    /// budget を消費しきった場合は Suspended を返す。
    pub fn step(&mut self, budget: usize) -> StepResult { ... }

    /// 完了まで一括実行（最大ステップ制限付き）
    pub fn run(&mut self, max_steps: usize) -> StepResult {
        self.step(max_steps)
    }
}
```

### 状態参照メソッド

```rust
impl NospaceVM {
    /// 実行完了済みか
    pub fn is_complete(&self) -> bool { self.completed }

    /// 総式評価回数
    pub fn total_steps(&self) -> usize { self.total_steps }

    /// stdout の内容を文字列として取得（テスト用）
    pub fn get_stdout_string(&self) -> String { ... }

    /// 戻り値（完了時のみ有効）
    pub fn return_value(&self) -> Option<i64> { self.return_value }

    /// トレース結果
    pub fn traced(&self) -> &BTreeMap<i64, i64> { &self.traced }

    /// stdout をフラッシュ
    pub fn flush(&mut self) { ... }
}
```

## 内部設計

### フレーム定義

AST の各構造に対応するフレームを定義する。
再帰インタプリタの各 `interpret_*` メソッドが1つのフレーム種別に対応する。

```rust
/// 実行フレーム
///
/// 再帰インタプリタの「今どの関数のどの行を実行中か」に対応する情報を保持する。
/// フレームスタックの末尾が現在実行中のフレーム。
enum Frame {
    /// グローバル初期化フレーム
    /// interpret_global() に対応
    GlobalInit {
        phase: GlobalInitPhase,
    },

    /// 関数呼び出しフレーム
    /// interpret_call_user_function_by_ref() に対応
    FunctionCall {
        /// 関数インデックス（root_scope.functions[idx]）
        func_idx: usize,
        /// 引数評価フェーズ: 評価済み引数値
        evaluated_args: Vec<i64>,
        /// 引数評価フェーズ: 次に評価する引数のインデックス
        next_arg_idx: usize,
        /// 本体実行中の文インデックス
        body_stmt_idx: usize,
        /// static 変数の有無
        has_static: bool,
        /// このフレームのスコープアドレス
        scope_addr: i64,
    },

    /// 文リスト実行フレーム
    /// interpret_statements() に対応
    Statements {
        /// 文リストへの参照（インデックスベースで AST にアクセス）
        context: StatementsContext,
        /// 次に実行する文のインデックス
        next_idx: usize,
        /// 最後の式の値（if/while 式の戻り値用）
        last_value: i64,
    },

    /// 式評価フレーム
    /// interpret_expression() に対応
    Expression {
        /// 評価する式への参照情報
        context: ExpressionContext,
        /// 式評価の進捗状態
        phase: ExpressionPhase,
    },

    /// while ループフレーム
    /// interpret_while_statement() に対応
    WhileLoop {
        context: WhileContext,
        phase: WhilePhase,
    },

    /// for ループフレーム
    /// interpret_for_statement() に対応
    ForLoop {
        context: ForContext,
        phase: ForPhase,
    },

    /// if 式フレーム
    /// interpret_if() に対応
    IfExpr {
        context: IfContext,
        phase: IfPhase,
    },

    /// ブロックスコープフレーム
    /// interpret_block() に対応
    BlockScope {
        context: BlockContext,
        /// ブロック内の文実行中のインデックス
        stmt_idx: usize,
        /// スコープアドレス
        scope_addr: i64,
    },

    /// 組み込み関数呼び出しフレーム
    BuiltinCall {
        kind: BuiltinFunctionKind,
        /// 評価済み引数
        evaluated_args: Vec<i64>,
        /// 次に評価する引数のインデックス
        next_arg_idx: usize,
    },
}
```

### フェーズ enum

各フレームの実行進捗を管理する:

```rust
/// グローバル初期化のフェーズ
enum GlobalInitPhase {
    /// static 変数初期化（root_statements 実行前）
    StaticInit { stmt_idx: usize },
    /// 関数内 static 初期化
    FunctionStaticInit { func_idx: usize, stmt_idx: usize },
    /// 非 static グローバル変数初期化
    RootStatements { stmt_idx: usize },
    /// main 関数呼び出し（GlobalInit の最後に FunctionCall フレームを積む）
    CallMain,
}

/// while ループのフェーズ
enum WhilePhase {
    /// 条件式を評価
    EvalCondition,
    /// ブロック進入済み、文を実行中
    ExecuteBody { stmt_idx: usize, scope_addr: i64 },
}

/// for ループのフェーズ
enum ForPhase {
    /// 初期化ブロック
    Init { stmt_idx: usize, scope_addr: i64 },
    /// 条件評価
    EvalCondition { init_scope_addr: i64 },
    /// 本体実行
    ExecuteBody { stmt_idx: usize, scope_addr: i64, init_scope_addr: i64 },
    /// ステップ実行
    ExecuteStep { stmt_idx: usize, scope_addr: i64, init_scope_addr: i64 },
}

/// if 式のフェーズ
enum IfPhase {
    /// 条件式を評価
    EvalCondition,
    /// then/else ブロック実行中
    ExecuteBlock { is_then: bool, stmt_idx: usize, scope_addr: i64 },
}

/// 式評価のフェーズ
enum ExpressionPhase {
    /// 単項演算: オペランド評価中
    Unary { op: Operator1 },
    /// 二項演算: 左辺評価中
    BinaryLeft { op: Operator2 },
    /// 二項演算: 右辺評価中（左辺の値を保持）
    BinaryRight { op: Operator2, left_value: i64 },
    /// 代入: 右辺評価中
    AssignRight { target: AssignTarget },
    /// 関数呼び出し: 引数評価中
    UserFuncArgs { func_ref: IdentifierRef, evaluated: Vec<i64>, next_idx: usize },
    /// 完了（値が value_stack に積まれた状態）
    Done,
}
```

### AST への参照管理

`Scope` を `NospaceVM` が所有するため、フレームから AST ノードへはインデックスベースでアクセスする。
`&` 参照は使わない（自己参照構造を回避）。

```rust
/// 文リストの位置を表すコンテキスト
///
/// Scope が所有する AST ツリー内の文リストを逆引きするためのインデックスチェーン。
/// 例: scope.functions[func_idx].block.statements[stmt_idx]
enum StatementsContext {
    /// 関数本体の文リスト: scope.functions[func_idx].block.statements
    FunctionBody { func_idx: usize },
    /// if の then ブロック
    /// 親の式コンテキストから辿る
    IfThenBlock { parent_expr: Box<ExpressionContext> },
    /// if の else ブロック
    IfElseBlock { parent_expr: Box<ExpressionContext> },
    /// while の本体
    WhileBody { parent_context: Box<StatementsContext>, parent_stmt_idx: usize },
    /// for の各パート
    ForInit { parent_context: Box<StatementsContext>, parent_stmt_idx: usize },
    ForCond { parent_context: Box<StatementsContext>, parent_stmt_idx: usize },
    ForStep { parent_context: Box<StatementsContext>, parent_stmt_idx: usize },
    ForBody { parent_context: Box<StatementsContext>, parent_stmt_idx: usize },
    /// ブロックスコープ式
    BlockScope { parent_expr: Box<ExpressionContext> },
    /// グローバル初期化の文リスト
    GlobalStaticInit,
    GlobalRootStatements,
    FunctionStaticInit { func_idx: usize },
}
```

> **設計ノート**: コンテキストチェーンの代わりに、各フレームが文リスト・式への生ポインタを持つ方法も検討可能。
> `Scope` は `NospaceVM` が所有し move しないため、`*const` ポインタは有効なまま保持される。
> ただし `unsafe` が必要になるため、まずはインデックスベースで実装し、パフォーマンスが問題になった場合に切り替える。

### 実行ループ

```rust
impl NospaceVM {
    pub fn step(&mut self, budget: usize) -> StepResult {
        if self.completed {
            return StepResult::Complete { return_value: self.return_value };
        }

        for _ in 0..budget {
            match self.execute_one_step() {
                ExecuteResult::Continue => {
                    self.total_steps += 1;
                }
                ExecuteResult::Complete(value) => {
                    self.completed = true;
                    self.return_value = value;
                    return StepResult::Complete { return_value: value };
                }
                ExecuteResult::Error(e) => {
                    return StepResult::Error(e);
                }
            }
        }

        StepResult::Suspended
    }

    /// 1ステップ（1式評価）の実行
    ///
    /// フレームスタックの末尾を見て、対応する処理を実行する。
    /// フレームが完了したら pop し、結果を value_stack に積む。
    fn execute_one_step(&mut self) -> ExecuteResult {
        let frame = match self.frames.last_mut() {
            Some(f) => f,
            None => return ExecuteResult::Complete(None),
        };

        match frame {
            Frame::GlobalInit { .. } => self.step_global_init(),
            Frame::FunctionCall { .. } => self.step_function_call(),
            Frame::Statements { .. } => self.step_statements(),
            Frame::Expression { .. } => self.step_expression(),
            Frame::WhileLoop { .. } => self.step_while(),
            Frame::ForLoop { .. } => self.step_for(),
            Frame::IfExpr { .. } => self.step_if(),
            Frame::BlockScope { .. } => self.step_block(),
            Frame::BuiltinCall { .. } => self.step_builtin_call(),
        }
    }
}
```

### ステップ実行の例: while ループ

```rust
fn step_while(&mut self) -> ExecuteResult {
    let frame = self.frames.last_mut().unwrap();
    let Frame::WhileLoop { context, phase } = frame else { unreachable!() };

    match phase {
        WhilePhase::EvalCondition => {
            // 条件式の評価フレームを積む
            // 条件式の評価結果が value_stack に積まれたら、
            // 次の step_while() 呼び出しで値を取り出して判定する
            // → 実際にはフレームの push/pop で制御
            todo!("条件式の Expression フレームを push")
        }
        WhilePhase::ExecuteBody { stmt_idx, scope_addr } => {
            // ブロック内の文を順次実行
            // 最後の文まで完了 → EvalCondition に戻る
            // Break → while フレームを pop
            // Return → Return flow を設定して while フレームを pop
            todo!("文実行のロジック")
        }
    }
}
```

### FlowControl（制御フロー伝播）

再帰版の `Flow` enum に対応する。スタックマシンでは `return` / `break` / `continue` を
フレーム pop 時に伝播する:

```rust
/// 制御フローの種別
enum FlowControl {
    /// return 文: 値を持って関数フレームまで巻き戻す
    Return(i64),
    /// break 文: ループフレームまで巻き戻す
    Break,
    /// continue 文: ループフレームまで巻き戻す（ループ先頭に戻る）
    Continue,
}
```

`execute_one_step()` の先頭で `flow` をチェックし、適切なフレームまで pop する:

```rust
fn execute_one_step(&mut self) -> ExecuteResult {
    // 制御フロー伝播: flow が設定されている場合、対象フレームまで pop
    if let Some(flow) = &self.flow {
        match flow {
            FlowControl::Return(val) => {
                // FunctionCall フレームまで pop（スコープ解放を含む）
                // FunctionCall に到達 → val を value_stack に積んで flow をクリア
            }
            FlowControl::Break => {
                // WhileLoop / ForLoop フレームまで pop
            }
            FlowControl::Continue => {
                // WhileLoop / ForLoop フレームまで pop（条件再評価へ）
            }
        }
    }
    // ... 通常のフレーム処理
}
```

## 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/interpreter/vm.rs` | **新規**: `NospaceVM`, `StepResult`, `Frame` 等の定義と実装 |
| `src/interpreter/mod.rs` | `mod vm;` の追加と `NospaceVM` / `StepResult` の re-export |
| `src/lib.rs` | `NospaceVM` の re-export |

## 既存 API との互換性

既存の再帰インタプリタ API は**一切変更しない**:

```rust
// 以下の関数はすべてそのまま残る
pub fn interpret(scope: &Scope) -> Result<Option<i64>, InterpretError>;
pub fn interpret_with_env(env: &mut Environment, scope: &Scope) -> Result<Option<i64>, InterpretError>;
pub fn interpret_func(scope: &Scope, func_name: &str) -> Result<Option<i64>, InterpretError>;
pub fn interpret_func_with_env(...) -> Result<Option<i64>, InterpretError>;
```

新しい `NospaceVM` は完全に独立したモジュールとして追加される。
CLI ではデフォルトで既存の再帰版インタプリタを使用し、WASM やステップ実行が必要な場合のみ `NospaceVM` を使用する。

## WhitespaceVM との対比表

| 観点 | WhitespaceVM | NospaceVM |
|------|------|------|
| 実行対象 | フラット命令列 | AST ツリー |
| 実行位置管理 | pc (プログラムカウンタ) | frames (フレームスタック) |
| データ管理 | data_stack + heap | value_stack + アロケータ (Environment) |
| コールスタック | call_stack (戻りアドレス) | FunctionCall フレーム |
| 1ステップの粒度 | 1命令 | 1式評価 |
| エラー型 | RuntimeError (WsRuntimeError) | InterpretError |
| I/O | StdinSource + stdout | Environment (stdin + stdout) |
| プログラム所有 | instructions: Vec<Instruction> | scope: Scope |

## WASM API 設計

### 方針

- `NospaceVM` を WASM から利用可能にする（`WasmNospaceVM` ラッパー）
- 既存の `run()` 関数（再帰インタプリタ `interpret_with_env` を使用）は WASM API から**削除**する
- `compile()`, `parse()`, `getOptions()` 等のコンパイル系 API はそのまま維持
- `WasmWhitespaceVM` もそのまま維持

### 現状の WASM API 構成と変更点

| API | 種別 | 変更 |
|-----|------|------|
| `run()` | トップレベル関数 | **削除** — 再帰インタプリタ使用のため |
| `compile()` | トップレベル関数 | 変更なし |
| `parse()` | トップレベル関数 | 変更なし |
| `compile_to_whitespace_string()` | ヘルパー関数 | 変更なし |
| `compile_to_mnemonic_string()` | ヘルパー関数 | 変更なし |
| `getOptions()` | メタデータ | 変更なし |
| `WasmWhitespaceVM` | VM クラス | 変更なし |
| `WasmNospaceVM` | VM クラス | **新規追加** |

### `run()` の削除

`api.rs` の `run()` 関数は `interpret_with_env()` （再帰インタプリタ）を使用しており、以下の理由で WASM API から削除する:

1. **メインスレッドブロック**: 完了まで制御が戻らず、UI をフリーズさせる
2. **`WasmNospaceVM`で代替可能**: `step()` ループで同等の動作を実現可能
3. **`max_expression_count` 超過で panic**: WASM 環境で回復不能なエラーになる

削除方法: `#[wasm_bindgen]` アトリビュートと関数自体を `api.rs` から削除する。

> **注**: `run()` の代替として利用者は `WasmNospaceVM` を使用する。
> ワンショット実行が必要な場合は以下のパターンで実現可能:
> ```javascript
> const vm = new WasmNospaceVM(source, stdin);
> while (true) {
>   const result = vm.step(100000);
>   if (result.status !== 'suspended') break;
> }
> const stdout = vm.flushStdout();
> ```

### WasmNospaceVM 設計

`WasmWhitespaceVM` と同パターンの WASM ラッパー。`src/wasm_api/nospace_vm.rs` に新規作成する。

#### 構造体

```rust
// src/wasm_api/nospace_vm.rs (新規)

use crate::interpreter::NospaceVM;

#[wasm_bindgen]
pub struct WasmNospaceVM {
    vm: NospaceVM,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}
```

#### コンストラクタ

```rust
#[wasm_bindgen]
impl WasmNospaceVM {
    /// nospace ソースコードから VM を構築する
    ///
    /// - `opt_passes`: 最適化パスの配列（省略可）
    /// - `ignore_debug`: デバッグ用組み込み関数を無視するか（省略可、デフォルト false）
    #[wasm_bindgen(constructor)]
    pub fn new(
        source: &str,
        stdin: &str,
        interactive: Option<bool>,
        opt_passes: Option<JsOptPassArray>,
        ignore_debug: Option<bool>,
    ) -> Result<WasmNospaceVM, JsValue> {
        // 1. pipeline::analyze_and_optimize() でソースを解析・最適化
        // 2. NospaceVM::from_scope(scope) で VM 構築
        // 3. stdin / stdout を設定
        // 4. interactive の場合は with_interactive_stdin() を適用
    }
}
```

> **WasmWhitespaceVM との差異**: `std_extensions` パラメータは不要（Whitespace コンパイル時のオプションであるため）。
> 代わりに `opt_passes` と `ignore_debug` を受け取る。

#### 実行メソッド

```rust
#[wasm_bindgen]
impl WasmNospaceVM {
    /// 指定ステップ数だけ実行する
    ///
    /// 戻り値: VmStepResult ({ status, error? })
    pub fn step(&mut self, budget: u32) -> JsVmStepResult {
        let result = self.vm.step(budget as usize);
        // StepResult → VmStepResult に変換
        // Suspended → { status: "suspended" }
        // Complete  → { status: "complete" }
        // Error     → { status: "error", error: "..." }
    }
}
```

> **注**: NospaceVM の StepResult には `WaitingForInput` がない（入力待ちは Whitespace VM 固有の概念）。
> 将来 interactive stdin を NospaceVM に追加する場合は `WaitingForInput` を追加する。

#### stdin メソッド（interactive モード用）

```rust
#[wasm_bindgen]
impl WasmNospaceVM {
    /// stdin にデータを追加する（interactive モード用）
    #[wasm_bindgen(js_name = "provideStdin")]
    pub fn provide_stdin(&mut self, data: &str) {
        self.vm.provide_stdin(data);
    }

    /// stdin のストリーム終端を通知する（interactive モード用）
    #[wasm_bindgen(js_name = "closeStdin")]
    pub fn close_stdin(&mut self) {
        self.vm.close_stdin();
    }
}
```

#### 状態参照メソッド

```rust
#[wasm_bindgen]
impl WasmNospaceVM {
    /// 実行完了済みか
    pub fn is_complete(&self) -> bool {
        self.vm.is_complete()
    }

    /// 総式評価回数
    pub fn total_steps(&self) -> usize {
        self.vm.total_steps()
    }

    /// 標準出力バッファの内容を取得しクリアする
    #[wasm_bindgen(js_name = "flushStdout")]
    pub fn flush_stdout(&mut self) -> String {
        let mut buf = self.stdout_buffer.borrow_mut();
        let text = String::from_utf8_lossy(&buf).to_string();
        buf.clear();
        text
    }

    /// 戻り値を取得（完了時のみ有効）
    #[wasm_bindgen(js_name = "getReturnValue")]
    pub fn get_return_value(&self) -> Option<i64> {
        self.vm.return_value()
    }

    /// トレース情報を取得
    #[wasm_bindgen(js_name = "getTraced")]
    pub fn get_traced(&self) -> JsNumberRecord {
        let traced: BTreeMap<String, f64> = self.vm.traced()
            .iter()
            .map(|(k, v)| (k.to_string(), *v as f64))
            .collect();
        serde_wasm_bindgen::to_value(&traced).unwrap().into()
    }
}
```

#### WasmWhitespaceVM との比較

| メソッド | WasmWhitespaceVM | WasmNospaceVM | 備考 |
|---------|------|------|------|
| `new(source, ...)` | ✓ (nospace→WS コンパイル) | ✓ (nospace→NospaceVM) | |
| `fromWhitespace()` | ✓ | — | WS ソース直接入力は NospaceVM に不要 |
| `fromWhitespaceInteractive()` | ✓ | — | 同上 |
| `step(budget)` | ✓ | ✓ | |
| `provideStdin(data)` | ✓ | ✓ | |
| `closeStdin()` | ✓ | ✓ | |
| `is_complete()` | ✓ | ✓ | |
| `total_steps()` | ✓ | ✓ | |
| `flushStdout()` | ✓ (`flush_stdout`) | ✓ | |
| `getReturnValue()` | — | ✓ | nospace 固有（main の戻り値） |
| `getTraced()` | ✓ (`get_traced`) | ✓ | |
| `pc()` | ✓ | — | nospace はフレームスタックベース |
| `getStack()` | ✓ | — | Whitespace 固有（データスタック） |
| `getHeap()` | ✓ | — | Whitespace 固有 |
| `callStackDepth()` | ✓ | — | 将来拡張で追加可能 |
| `currentInstruction()` | ✓ | — | Whitespace 固有 |
| `disassemble()` | ✓ | — | Whitespace 固有 |

#### TypeScript 型定義の追加

`types.rs` の `TS_TYPES` に追加する型定義:

```typescript
// types.rs の TS_TYPES に追記
interface NospaceVmStepResult {
    status: "suspended" | "complete" | "error";
    error?: string;
}
```

> **注**: `WasmWhitespaceVM` と同じ `VmStepResult` を使うことも可能だが、
> `waiting_for_input` と `inputType` が NospaceVM には不要なため、
> 別の型 `NospaceVmStepResult` を定義する方が型安全である。
> ただし、統一性を優先して `VmStepResult` を共用する選択もあり得る。
> 初期実装では `VmStepResult` を共用し、必要に応じて分離する。

### 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/wasm_api/nospace_vm.rs` | **新規**: `WasmNospaceVM` ラッパー |
| `src/wasm_api/mod.rs` | `mod nospace_vm;` の追加 |
| `src/wasm_api/api.rs` | `run()` 関数の削除、`interpret_with_env` の import 削除 |
| `src/wasm_api/types.rs` | `RunResultOk` / `JsRunResult` 関連の削除（`run()` 廃止に伴う） |
| `src/lib.rs` | `NospaceVM` / `StepResult` の re-export 追加（Phase 1 で対応済みの想定） |

> **`types.rs` の変更について**: `RunResultOk` は `run()` のみで使用されるため、`run()` 削除時に未使用になる。
> ただし、TypeScript 型定義 `RunResult` は外部利用者が参照している可能性があるため、
> 非推奨（deprecated）として残すか完全削除するかは実装時に判断する。

### モジュール構成（変更後）

```
src/wasm_api/
  mod.rs              # モジュール宣言
  types.rs            # TypeScript 型定義・Serde 構造体
  pipeline.rs         # 共通コンパイルパイプライン
  api.rs              # トップレベル API (compile, parse, getOptions)  ← run() 削除
  whitespace_vm.rs    # WasmWhitespaceVM (変更なし)
  nospace_vm.rs        # WasmNospaceVM (新規)
```

## 使用例

### native (CLI / テスト)

```rust
let scope = semantic_analyze(&stmts)?;
let mut vm = NospaceVM::from_scope(scope)?;

loop {
    match vm.step(10000) {
        StepResult::Complete { return_value } => {
            println!("Done: {:?}", return_value);
            break;
        }
        StepResult::Suspended => {
            continue;
        }
        StepResult::Error(e) => {
            eprintln!("Error: {}", e);
            break;
        }
    }
}
```

### WASM

```javascript
const vm = new WasmNospaceVM(source, stdin);

function runChunk() {
  const result = vm.step(10000);
  if (result.status === 'suspended') {
    requestAnimationFrame(runChunk);
  } else {
    const stdout = vm.flushStdout();
    handleResult(result, stdout);
  }
}
runChunk();
```

## 設計上のトレードオフ

### ステップ粒度

| 粒度 | 利点 | 欠点 |
|------|------|------|
| 1式評価 | 既存の `increment_expression_count` と同等 | 1ステップの所要時間にばらつき（関数呼び出しは長い） |
| 1文実行 | ばらつき小 | if/while 式が文として扱えない |
| 1 AST ノード | 最も均一 | フレーム数が膨大 |

**方針: 1式評価**を採用。`WhitespaceVM` の「1命令」に対応する自然な粒度であり、
既存の `expression_count` メトリクスとも整合する。

### AST 参照方式

| 方式 | 利点 | 欠点 |
|------|------|------|
| インデックスチェーン | safe Rust のみ | コンテキストのネスト時に辿るコストがある |
| 生ポインタ (`*const`) | O(1) アクセス | `unsafe` が必要 |
| `Rc<ExecExpression>` | 安全 + O(1) | AST 全体を `Rc` に変換する必要あり、既存コード変更大 |

**方針: インデックスチェーン**を初期実装として採用。
パフォーマンスが問題になった場合にポインタ方式に移行する。
