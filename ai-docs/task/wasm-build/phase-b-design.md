# Phase B: nospace ステップ実行インタプリタ API

## 概要

nospace ソースコードを直接ステップ実行する中断可能インタプリタを構築し、WASM API として公開する。

Phase A（Whitespace VM 経由の実行）と異なり、nospace の AST レベルでステップ実行するため、
変数名・関数名・ソースコード上の実行位置など、高レベルなデバッグ情報を提供できる。

```
[nospace ソース]
    ↓ パース + 意味解析
[Scope (意味解析結果)]
    ↓ WasmInterpreterSession::new()
[Session インスタンス]
    ↓ session.step() を繰り返し呼び出し
[実行完了 / 中断中 / エラー]
```

## Phase A との比較

| 項目 | Phase A (WS VM) | Phase B (nospace interpreter) |
|------|-----------------|-------------------------------|
| 実行対象 | Whitespace 命令列 | nospace の Scope (意味解析結果) |
| ステップ粒度 | WS 命令単位 | 式評価単位 |
| デバッグ情報 | スタック・ヒープ・PC | 変数名・値・コールスタック・ソース位置 |
| 実装難易度 | 低（VM は既存） | 高（インタプリタの大規模改修） |
| 実行速度 | WS VM のオーバーヘッド | ネイティブインタプリタ |
| Web UI 適性 | 低レベルデバッガ向け | 高レベルデバッガ / Playground 向け |

**使い分け**:
- Phase A: Whitespace 命令レベルの動作を見たいとき、WS の学習ツールとして
- Phase B: nospace プログラムのデバッグ、Playground での実行・学習ツールとして

## 前提条件・依存タスク

| タスク | 状態 | 必要度 | 説明 |
|--------|------|--------|------|
| `suspendable-interpreter` | 未着手 | **必須** | インタプリタの中断・再開機能 |
| `wasm-build` Phase 1（ビルド基盤） | 未着手 | 必須 | Cargo.toml + wasm-bindgen 設定 |
| `interpreter` ユニットテスト | 一部完了 | 推奨 | リファクタ前の動作保証 |

### suspendable-interpreter の要約

`suspendable-interpreter/` タスクで設計済みの **アプローチ B（Yield 伝播 + 継続保存）** を採用する。

現在のインタプリタは再帰呼び出しで実行状態を管理しており、中断・再開ができない。
`Flow::Yield` を追加し、ステップバジェット到達時に呼び出しチェーンを巻き戻して中断、
`Continuation` 情報で再開時に中断地点まで復帰する方式。

詳細は [../suspendable-interpreter/detailed-design.md](../suspendable-interpreter/detailed-design.md) を参照。

## 新規 WASM API

### WasmInterpreterSession（ステートフルセッション）

```rust
// src/wasm_api.rs に追加

/// nospace インタプリタのステップ実行セッション
///
/// Scope と実行状態を保持し、step() 呼び出しで段階的に実行する。
#[wasm_bindgen]
pub struct WasmInterpreterSession {
    session: InterpreterSession<'static>,
    // Scope を所有する（ライフタイム問題の回避）
    _scope: Box<Scope>,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}
```

### ライフタイム問題への対処

`InterpreterSession<'a>` は `&'a Scope` を参照するが、WASM API ではオブジェクトの所有権を
JS 側に渡す必要がある。`Scope` を `Box<Scope>` として所有し、unsafe で自己参照を構築するか、
あるいは `Scope` を `Arc` でラップする方法がある。

**推奨方式: Scope の所有権をセッションに含める**

```rust
/// self-referential struct を安全に構築するためのヘルパー
///
/// Scope を Box で保持し、そこへの参照を InterpreterSession に渡す。
/// Drop 順序を正しく管理するため、session を先に Drop する。
pub struct OwnedInterpreterSession {
    /// session は scope への参照を持つため、先に Drop すること
    session: Option<InterpreterSession<'static>>,
    /// session が参照する Scope
    scope: Pin<Box<Scope>>,
    env: Environment,
}
```

あるいは、より安全な代替として `InterpreterSession` が `Scope` を所有する
新しいバリアント `OwnedInterpreterSession` を interpreter 側に実装する。

```rust
// src/interpreter/session.rs

/// Scope を所有するインタプリタセッション（WASM 用）
pub struct OwnedInterpreterSession {
    scope: Box<Scope>,
    env: Environment,
    continuation: Option<Continuation>,
    step_budget: usize,
}

impl OwnedInterpreterSession {
    pub fn new(scope: Scope, env: Environment, step_budget: usize) -> Self { ... }
    pub fn step(&mut self) -> StepResult { ... }
    pub fn env(&self) -> &Environment { ... }
    pub fn env_mut(&mut self) -> &mut Environment { ... }
}
```

### コンストラクタ

```rust
#[wasm_bindgen]
impl WasmInterpreterSession {
    /// nospace ソースをパース・解析し、実行セッションを作成する
    ///
    /// step_budget: 1回の step() で実行する最大式評価回数
    #[wasm_bindgen(constructor)]
    pub fn new(
        source: &str,
        stdin: &str,
        step_budget: u32,
    ) -> Result<WasmInterpreterSession, JsValue>;
}
```

### 実行制御メソッド

```rust
#[wasm_bindgen]
impl WasmInterpreterSession {
    /// step_budget 分だけ実行を進める
    ///
    /// 戻り値: {
    ///   status: "suspended" | "complete" | "error",
    ///   returnValue?: number,  // status="complete" 時
    ///   error?: string,        // status="error" 時
    /// }
    pub fn step(&mut self) -> JsValue;

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool;

    /// 総式評価回数
    pub fn expression_count(&self) -> usize;
}
```

### デバッグ情報メソッド

```rust
#[wasm_bindgen]
impl WasmInterpreterSession {
    /// 現在のスコープのローカル変数一覧
    ///
    /// 戻り値: [{ name: string, value: number }]
    pub fn get_local_variables(&self) -> JsValue;

    /// グローバル変数一覧
    ///
    /// 戻り値: [{ name: string, value: number }]
    pub fn get_global_variables(&self) -> JsValue;

    /// コールスタック（関数呼び出し履歴）
    ///
    /// 戻り値: [{ functionName: string, line?: number }]
    pub fn get_call_stack(&self) -> JsValue;

    /// 標準出力バッファの内容を取得しクリアする
    pub fn flush_stdout(&mut self) -> String;

    /// トレース情報を取得
    ///
    /// 戻り値: { [key: string]: number }
    pub fn get_traced(&self) -> JsValue;
}
```

## JS 側の利用例

### Playground 実行

```javascript
import { WasmInterpreterSession } from './pkg/nospace20.js';

const source = `
func: main() {
  let: x;
  x = 0;
  while: x < 10 {
    __puti(x);
    __putc(10);
    x = x + 1;
  };
}
`;

const session = new WasmInterpreterSession(source, "", 5000);

function runChunk() {
  const result = session.step();
  
  // 出力を回収
  const output = session.flush_stdout();
  if (output) {
    document.getElementById('output').textContent += output;
  }
  
  if (result.status === 'suspended') {
    requestAnimationFrame(runChunk);
  } else if (result.status === 'complete') {
    console.log('Done. Return:', result.returnValue);
  } else {
    console.error('Error:', result.error);
  }
}

runChunk();
```

### デバッガ UI との統合

```javascript
const session = new WasmInterpreterSession(source, stdin, 1);

function stepOne() {
  const result = session.step();
  
  // デバッグ情報更新
  updateVariablesPanel(session.get_local_variables());
  updateGlobalsPanel(session.get_global_variables());
  updateCallStackPanel(session.get_call_stack());
  updateExprCountDisplay(session.expression_count());
  
  const output = session.flush_stdout();
  if (output) appendOutput(output);
  
  return result;
}

document.getElementById('step-btn').onclick = stepOne;

document.getElementById('run-btn').onclick = () => {
  // budget を大きくして一括実行
  const batchSession = new WasmInterpreterSession(source, stdin, 100000);
  function chunk() {
    const result = batchSession.step();
    if (result.status === 'suspended') {
      const output = batchSession.flush_stdout();
      if (output) appendOutput(output);
      requestAnimationFrame(chunk);
    } else {
      const output = batchSession.flush_stdout();
      if (output) appendOutput(output);
      handleResult(result);
    }
  }
  chunk();
};
```

## 実装上の考慮事項

### suspendable-interpreter の実装順序

Phase B の WASM API を実装する前に、`suspendable-interpreter` タスクの Phase 1〜3 を完了する必要がある。

| Phase | 内容 | API 公開への影響 |
|-------|------|-----------------|
| Phase 1 | 型と API の整備 (`InterpreterSession`, `StepResult`) | WASM ラッパーの型設計 |
| Phase 2 | Yield 導入（panic → Yield 返却） | 中断機能の基盤 |
| Phase 3 | 継続情報の保存・復元 | 再開機能の実現 |
| Phase 4 | テスト | 品質保証 |

### デバッグ情報の取得

現在のインタプリタは `LocalEnvironment` で変数値を管理しているが、変数名のマッピングは
`Scope` の `Block` 定義にある。デバッグ情報を提供するには以下が必要：

1. 現在実行中の `Block` (scope_depth) の特定
2. `Block.identifiers` から変数名を取得
3. `LocalEnvironment` から変数値を取得
4. 両者を突合して `{ name, value }` ペアを構築

これは `InterpreterSession` に `get_variables()` メソッドとして実装する。

```rust
// src/interpreter/session.rs

impl InterpreterSession<'_> {
    /// 現在のスコープのローカル変数を名前・値のペアで返す
    pub fn get_local_variables(&self) -> Vec<(String, i64)> {
        // continuation から現在の scope_depth / block を特定
        // scope の identifiers と local_env の values を突合
        todo!()
    }

    /// グローバル変数を名前・値のペアで返す
    pub fn get_global_variables(&self) -> Vec<(String, i64)> {
        // env.global_variables と scope.root_block.identifiers を突合
        todo!()
    }
}
```

### 段階的実装の提案

デバッグ情報の全てを初期実装に含める必要はない。以下の優先度で段階的に実装する：

| 優先度 | 機能 | 理由 |
|--------|------|------|
| 高 | `step()`, `is_complete()`, `flush_stdout()` | 最小限の実行機能 |
| 高 | `get_traced()` | テスト検証に必要 |
| 中 | `get_local_variables()`, `get_global_variables()` | Playground UI 向け |
| 中 | `get_call_stack()` | デバッガ UI 向け |
| 低 | ソース位置のハイライト | エディタ統合向け |

## フェーズ計画

### Step B-1: suspendable-interpreter の実装

`suspendable-interpreter/` タスクの Phase 1〜4 を実施する。
これは Phase B の前提条件であり、最も工数が大きいパート。

- [ ] `InterpreterSession` / `StepResult` 型定義
- [ ] `Flow::Yield` の導入と伝播
- [ ] `Continuation` による状態保存・復元
- [ ] 既存テストがパスすることの確認
- [ ] ステップ実行のユニットテスト

### Step B-2: OwnedInterpreterSession の実装

- [ ] `OwnedInterpreterSession` 構造体の実装（Scope 所有版）
- [ ] `lib.rs` に公開 API 追加

### Step B-3: WASM API — WasmInterpreterSession

- [ ] `WasmInterpreterSession::new()` 実装
- [ ] `step()`, `is_complete()`, `expression_count()` 実装
- [ ] `flush_stdout()`, `get_traced()` 実装

### Step B-4: デバッグ情報 API

- [ ] `get_local_variables()` 実装
- [ ] `get_global_variables()` 実装
- [ ] `get_call_stack()` 実装

### Step B-5: テスト・検証

- [ ] Node.js でのスモークテスト（セッション作成 → step → 結果確認）
- [ ] 既存テストケースの WASM 経由実行・結果照合
- [ ] Phase A との結果一致確認
