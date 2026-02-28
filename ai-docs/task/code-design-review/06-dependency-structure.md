# モジュール間依存構造の改善

## 問題 1: whitespace → compiler_ws の依存方向

### 現状

```
compiler_ws
    ├── instruction.rs  (Instruction 型定義)
    ├── types.rs        (WsNumber, WsChar, LabelId 型定義)
    └── ...

whitespace
    ├── mod.rs          (compiler_ws::instruction::Instruction を re-export)
    ├── parser.rs       (compiler_ws の型を使用)
    └── interpreter.rs  (compiler_ws の型を使用)
```

`whitespace` モジュール（Whitespace パーサ + VM）は本来 `compiler_ws`（nospace → Whitespace コンパイラ）とは独立した存在であるべきだが、`Instruction` 型等が `compiler_ws` に定義されているため、逆方向の依存が発生している。

### 改善案

共有型を `base` モジュールまたは新規の `ws_types` モジュールに移動:

```
base/
    └── ws_types.rs     # Instruction, WsNumber, WsChar, LabelId

compiler_ws/            # nospace → WS コンパイラ (base::ws_types を使用)
whitespace/             # WS パーサ + VM (base::ws_types を使用)
```

```rust
// src/base/ws_types.rs
pub enum Instruction { ... }
pub type WsNumber = i64;
pub type WsChar = u8;
pub type LabelId = usize;
pub struct WsProgram { ... }
```

これにより、`whitespace` と `compiler_ws` は `base` にのみ依存し、互いに独立する。

### 影響範囲

- `src/compiler_ws/instruction.rs` → `src/base/ws_types.rs` に移動
- `src/compiler_ws/types.rs` の共有型を移動
- `src/compiler_ws/` 内の `use` パスを更新
- `src/whitespace/` 内の `use` パスを更新
- `src/lib.rs` の re-export パスを更新

## 問題 2: unsafe キャストの除去

### 現状

`src/whitespace/interpreter.rs` の `get_stdout_string()` に危険な `unsafe` キャストがある:

```rust
pub fn get_stdout_string(&self) -> Option<String> {
    let stdout_ref: &Box<dyn Write> = &self.stdout;
    let bytes: &Vec<u8> =
        unsafe { &*(stdout_ref as *const Box<dyn Write> as *const Box<Vec<u8>>) };
    Some(String::from_utf8(bytes.clone()).unwrap())
}
```

`stdout` が `Vec<u8>` でない場合に未定義動作を引き起こす。

### 改善案 A: Any トレイトによるダウンキャスト

```rust
use std::any::Any;

pub struct WhitespaceVM {
    stdout: Box<dyn Write + Any>,
    // ...
}

pub fn get_stdout_string(&self) -> Option<String> {
    let any_ref = &self.stdout as &dyn Any;
    if let Some(buf) = any_ref.downcast_ref::<Box<Vec<u8>>>() {
        Some(String::from_utf8(buf.clone()).ok()?)
    } else {
        None
    }
}
```

ただし `dyn Write + Any` はオブジェクト安全性の制約がある。

### 改善案 B: テスト専用の出力チャネル

```rust
pub struct WhitespaceVM {
    stdout: Box<dyn Write>,
    /// テスト用: stdout の内容を保持するバッファ
    stdout_capture: Option<Rc<RefCell<Vec<u8>>>>,
}

impl WhitespaceVM {
    pub fn with_stdout_capture() -> Self {
        let buf = Rc::new(RefCell::new(Vec::new()));
        let writer = SharedWriter(Rc::clone(&buf));
        Self {
            stdout: Box::new(writer),
            stdout_capture: Some(buf),
            // ...
        }
    }

    pub fn get_stdout_string(&self) -> Option<String> {
        self.stdout_capture.as_ref().map(|buf| {
            String::from_utf8(buf.borrow().clone()).unwrap()
        })
    }
}
```

**推奨**: 改善案 B。`unsafe` を完全に除去でき、型安全。

## 問題 3: CodeGenContext のフィールド過多

### 現状

`src/compiler_ws/context.rs` の `CodeGenContext` は 18 フィールドを持ち、以下を兼務:

- ラベル管理
- 変数スコープ管理
- ループスタック管理
- static 変数オフセット管理
- ソース位置追跡
- コンパイルオプション保持

`enter_function` で構造体全体を手動コピーしており、フィールド追加時にバグが入りやすい。

### 改善案

責務ごとにサブ構造体に分割:

```rust
pub struct CodeGenContext {
    labels: LabelManager,          // ラベル ID 管理
    variables: VariableScope,      // スコープスタック + 変数マッピング
    loops: LoopStack,              // break/continue ラベル
    statics: StaticManager,        // static 変数オフセット
    source: SourceTracker,         // 位置情報
    options: CodeGenOptions,       // debug_ext, alloc_ext 等
}
```

`enter_function` では `variables` と `loops` のみリセットし、他はそのまま共有。

## 依存関係図（理想形）

```
                  base
                 / | \
                /  |  \
               /   |   \
   token_parser    |    ws_types
        |          |    /     \
   tree_parser     |  compiler_ws  whitespace
        |          |
  semantic_analyzer
        |
     optimizer
        |
   interpreter
```

現状からの差分:
- `ws_types` を `base` 配下に新設
- `whitespace` → `compiler_ws` の依存を解消
- `compiler_ws` → `base/ws_types` に依存方向を修正

## Progress

### 実施済み

- **問題 2 (unsafe キャストの除去)**:
  - `WhitespaceVM` に `stdout_capture: Option<Rc<RefCell<Vec<u8>>>>` フィールドを追加
  - `from_instructions` で `SharedWriter` + `stdout_capture` を使用し、デフォルト stdout をキャプチャ可能に
  - `get_stdout_string` を type-safe な実装に変更（`stdout_capture` 経由で取得）
  - `with_io` / `with_stdout` でカスタム stdout を設定した場合は `stdout_capture = None` に
  - `with_stdin` メソッドを新規追加（stdin のみ設定、stdout_capture を維持）
  - `src/interpreter/exec.rs` テスト内の unsafe キャストも同様に `SharedWriter` ベースに修正
  - alloc_runtime テスト、whitespace_direct_test、whitespace_self_base テストを更新

### 未実施（モジュール構造変更のため除外）

- **問題 1 (ws_types の base 移動)**: モジュール分割・依存方向の変更は今回のスコープ外
- **問題 3 (CodeGenContext 分割)**: 構造体の内部分割は今回のスコープ外
