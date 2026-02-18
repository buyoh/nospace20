# Phase 1: WhitespaceVM に strict-heap モードを追加

## 対象ファイル

| ファイル | 変更内容 | 規模 |
|---------|---------|------|
| `src/whitespace/interpreter.rs` | `strict_heap` フラグと builder メソッド追加、`heap_retrieve` の変更 | 小 |
| `src/whitespace/mod.rs` | （変更不要、公開 API は変わらない） | - |

## 設計

### WhitespaceVM 構造体への追加

```rust
pub struct WhitespaceVM {
    // ... 既存フィールド ...

    /// 未初期化ヒープアクセスをエラーにするか（wsc のデフォルト動作と同等）
    strict_heap: bool,
}
```

### builder メソッド

```rust
impl WhitespaceVM {
    /// strict-heap モードを有効にして構築
    /// 有効時、Store されていないアドレスへの Retrieve は UninitializedHeap エラーになる
    pub fn with_strict_heap(mut self, enabled: bool) -> Self {
        self.strict_heap = enabled;
        self
    }
}
```

### `heap_retrieve` の変更

現在:
```rust
fn heap_retrieve(&self, addr: i64) -> Result<i64, RuntimeError> {
    // 未初期化アドレスは 0 を返す（Whitespace の一般的な挙動）
    Ok(*self.heap.get(&addr).unwrap_or(&0))
}
```

変更後:
```rust
fn heap_retrieve(&self, addr: i64) -> Result<i64, RuntimeError> {
    match self.heap.get(&addr) {
        Some(&val) => Ok(val),
        None => {
            if self.strict_heap {
                Err(RuntimeError::UninitializedHeap(addr))
            } else {
                Ok(0)
            }
        }
    }
}
```

### `InputChar` / `InputNumber` のヒープ書き込み

`InputChar` と `InputNumber` は `self.heap.insert(addr, val)` で直接ヒープに書き込んでいるが、これは `heap_store` を経由していない。strict-heap モードにおいてはこれらの書き込みも「初期化済み」として扱うべきであり、現在の実装で問題ない（`HashMap::insert` で書き込まれるため `heap_retrieve` で見つかる）。

### コンストラクタの初期値

`from_instructions` で `strict_heap: false` を設定（デフォルト無効）。

## テスト

### Unit テスト追加（`src/whitespace/interpreter.rs` 内）

```rust
#[test]
fn test_strict_heap_uninitialized_error() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Retrieve,            // 未初期化
        Instruction::Exit,
    ])
    .unwrap()
    .with_strict_heap(true);
    let result = vm.run(100);
    assert_eq!(result, StepResult::Error(RuntimeError::UninitializedHeap(100)));
}

#[test]
fn test_strict_heap_initialized_ok() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Push(WsNumber(42)),  // value
        Instruction::Store,
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap()
    .with_strict_heap(true);
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[42]);
}

#[test]
fn test_non_strict_heap_uninitialized_returns_zero() {
    // 既存動作の確認
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(100)),
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[0]);
}
```

## 既存の RuntimeError::UninitializedHeap

`RuntimeError::UninitializedHeap(i64)` は既に定義されているが、現在どこからも生成されていない。この変更で実際に使用されるようになる。

## 更新履歴

- 2026-02-18: 初版作成
