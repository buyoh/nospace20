# Phase 6: ランダム初期化モード

## 概要

未定義変数にアクセスされた際にランダムな整数を返すモードを追加する。テスト時に有効化することで、初期値 0 に依存したバグを検出できる。

## 設計

### 2つのレイヤーでの対応

| レイヤー | 現在の挙動 | ランダムモードの挙動 |
|---------|-----------|-------------------|
| **nospace インタプリタ** | `vec![0; count]` | `vec![random(); count]` |
| **Whitespace VM** | `heap.get(&addr).unwrap_or(&0)` → `0` | `heap.get(&addr).unwrap_or(&random())` → ランダム値 |

### nospace インタプリタ

#### Environment への設定追加

```rust
// src/interpreter/mod.rs (Environment 構造体)
pub struct Environment {
    pub global_variables: Vec<i64>,
    pub function_static_storage: HashMap<usize, Vec<i64>>,
    // 追加:
    pub randomize_uninit: bool,
}
```

#### 変数領域確保時のランダム初期化

```rust
// src/interpreter/exec.rs - new_func
fn fill_value(randomize: bool) -> i64 {
    if randomize {
        // 決定論的でない値を返す（0 以外であれば初期値依存のバグを検出しやすい）
        // 簡易実装: アドレスのハッシュやカウンタベースでもよい
        random_uninit_value()
    } else {
        0
    }
}

let mut variables: Vec<i64> = (0..func.block.scope.variable_count)
    .map(|_| fill_value(env.randomize_uninit))
    .collect();
```

同様に `enter_block`, `interpret_global`, `initialize_function_statics` でも適用。

#### ランダム値生成

テスト再現性のため、**シード固定の擬似乱数**を使用する。

```rust
use std::cell::RefCell;

thread_local! {
    static UNINIT_COUNTER: RefCell<u64> = RefCell::new(0);
}

/// 未初期化変数用のフィル値を生成
/// 0 でない値を返すことで、初期値 0 への暗黙依存を検出する
fn random_uninit_value() -> i64 {
    UNINIT_COUNTER.with(|c| {
        let mut count = c.borrow_mut();
        *count = count.wrapping_add(1);
        // 簡易ハッシュ: 0 を避けつつ決定論的な非自明値を生成
        let v = (*count).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v as i64
    })
}
```

利点:
- 外部 crate（`rand`）への依存なし
- テスト間でカウンタがリセットされないため、毎回異なる値が使われる
- 再実行時にも決定論的（同じ実行順序なら同じ値）

### Whitespace VM

#### WhitespaceVM への設定追加

```rust
pub struct WhitespaceVM {
    // ... 既存フィールド ...

    /// 未初期化ヒープアクセス時にランダム値を返すか
    randomize_heap: bool,
}
```

#### builder メソッド

```rust
impl WhitespaceVM {
    /// ランダムヒープモードを有効にして構築
    /// 有効時、Store されていないアドレスへの Retrieve はランダム値を返す
    pub fn with_randomize_heap(mut self, enabled: bool) -> Self {
        self.randomize_heap = enabled;
        self
    }
}
```

#### `heap_retrieve` の変更

```rust
fn heap_retrieve(&self, addr: i64) -> Result<i64, RuntimeError> {
    match self.heap.get(&addr) {
        Some(&val) => Ok(val),
        None => {
            if self.strict_heap {
                Err(RuntimeError::UninitializedHeap(addr))
            } else if self.randomize_heap {
                // アドレスからの決定論的な非自明値
                Ok(random_heap_fill(addr))
            } else {
                Ok(0)
            }
        }
    }
}

/// 未初期化ヒープのフィル値（決定論的）
fn random_heap_fill(addr: i64) -> i64 {
    // アドレスベースのハッシュ（同じアドレスなら同じ値）
    (addr as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) as i64
}
```

注: `strict_heap` が `randomize_heap` より優先される。両方有効の場合はエラーとなる。

### フラグの優先度

```
strict_heap=true  → UninitializedHeap エラー（最優先）
randomize_heap=true → ランダム値を返す
どちらも false     → 0 を返す（従来動作）
```

## テスト基盤との統合

### test-manifest.yaml

Phase 3 と同様に新しいテストターゲットを追加:

- `whitespace-self-randomize`: ランダムヒープモードでの実行バリアント
- `interpreter-randomize`: ランダム初期化モードでの実行バリアント

ただし、テストバリアントが増えすぎる懸念があるため、**初期は CI でのみ実行するか、手動実行用のテストとして追加**する方が現実的かもしれない。

### 代替案: テストランナーレベルでのフラグ

test-manifest.yaml の新ターゲットではなく、テスト関数内にフラグを持たせる方式も検討可能:

```rust
fn test_whitespace_self_base_debug(test_name: &str, debug_ext: bool) {
    // 既存のテスト（0 初期化）
    run_test(test_name, debug_ext, false);

    // ランダム初期化で再実行
    run_test(test_name, debug_ext, true);
}
```

この方式なら test-manifest.yaml の変更は不要だが、テスト時間が2倍になる。

## 対象ファイル

| ファイル | 変更内容 | 規模 |
|---------|---------|------|
| `src/interpreter/mod.rs` | `Environment` に `randomize_uninit` フラグ追加、初期化処理変更 | 中 |
| `src/interpreter/exec.rs` | `new_func`, `enter_block` でランダム初期化 | 中 |
| `src/whitespace/interpreter.rs` | `randomize_heap` フラグ、`heap_retrieve` 変更 | 小 |
| `tests/code_test.rs` | ランダムモードのテストヘルパー追加 | 中 |
| `build.rs` | （テストバリアント生成を追加する場合） | 中 |

## 依存関係

- Phase 5（変数初期値の仕様変更）と同時に実装するのが効果的
- Phase 1（strict-heap モード）は独立して先行実装可能

## 更新履歴

- 2026-02-18: 初版作成
