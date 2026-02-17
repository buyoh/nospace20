# Phase 2: Whitespace VM 変更設計

## 目標

`WhitespaceVM` に拡張 API の有効/無効を制御するフラグを追加し、`--std-ext debug` 指定時のみ負ヒープアドレスによる拡張 API を有効化する。

## 変更対象ファイル

| ファイル | 変更内容 | 規模 |
|---|---|---|
| `src/whitespace/interpreter.rs` | `WhitespaceVM` に `debug_ext` フラグ追加、`heap_store` を条件分岐 | 小 |
| `src/bin/whitespace20.rs` | `--std-ext debug` を VM に渡す | 小 |

## 詳細設計

### 1. `WhitespaceVM` へのフラグ追加

```rust
pub struct WhitespaceVM {
    // ... 既存フィールド ...

    /// デバッグ拡張 API が有効か (--std-ext debug)
    debug_ext: bool,
}
```

初期値は `false`。

### 2. ビルダーメソッド追加

```rust
impl WhitespaceVM {
    /// デバッグ拡張を有効にして構築
    pub fn with_debug_ext(mut self, enabled: bool) -> Self {
        self.debug_ext = enabled;
        self
    }
}
```

既存の `with_io` と同様のビルダーパターンを使用する。

### 3. `heap_store` の条件分岐

```rust
fn heap_store(&mut self, addr: i64, val: i64) -> Result<(), RuntimeError> {
    if self.debug_ext {
        match addr {
            -1 => {
                // __trace(val)
                let traced = &mut self.traced;
                if let Some(v) = traced.get_mut(&val) {
                    *v += 1;
                } else {
                    traced.insert(val, 1);
                }
                return Ok(());
            }
            -2 => {
                // __assert(val): val == 0 ならエラー
                if val == 0 {
                    return Err(RuntimeError::AssertionFailed(val));
                }
                return Ok(());
            }
            -3 => {
                // __assert_not(val): val != 0 ならエラー
                if val != 0 {
                    return Err(RuntimeError::AssertionFailed(val));
                }
                return Ok(());
            }
            _ => {}
        }
    }
    // 通常のヒープ書き込み（debug_ext 無効時、または上記にマッチしないアドレス）
    self.heap.insert(addr, val);
    Ok(())
}
```

### 4. `whitespace20.rs` の変更

```rust
fn main() {
    let args = Args::parse();

    let target_extensions: Vec<TargetExtension> =
        args.std_ext.into_iter().map(|e| e.into()).collect();
    let debug_ext = target_extensions.contains(&TargetExtension::Debug);

    // ...

    let mut vm = match WhitespaceVM::from_source(&source) {
        Ok(vm) => vm.with_debug_ext(debug_ext),
        // ...
    };

    // ...
}
```

変更点:
- `_target_extensions` のプレフィックス `_` を除去
- `debug_ext` フラグを算出
- `vm.with_debug_ext(debug_ext)` をチェーンで呼び出し

### 5. テスト時の互換性

`lib.rs` のテスト用 API（`compile_to_whitespace` → `WhitespaceVM` 実行のフロー）では、`--std-ext debug` が指定されていない場合は拡張 API が無効になる。

テスト (`code_test.rs`) でデバッグ拡張を使ったテストが必要な場合:
- `WhitespaceVM::from_source(&ws_code).with_debug_ext(true)` で有効化可能

## 注意事項

- `TargetExtension` に `PartialEq` derive が必要（既に `Eq` がある）。`contains()` を使用するため。
- `debug_ext: false` がデフォルトなので、既存コードへの影響はない。
- 現在 VM は拡張 API を常に有効にしているため、この変更は **挙動の後方非互換変更** となる。ただし、拡張 API は仕様上 `--std-ext debug` 必須なので、正しい挙動に修正するものである。
