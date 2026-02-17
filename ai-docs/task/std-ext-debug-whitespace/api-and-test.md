# Phase 3: 公開 API 変更とテスト計画

## 目標

1. 公開 API (`lib.rs`) にデバッグ拡張情報を渡せるようにする
2. CLI (`nospace20`, `whitespace20`) で `--std-ext debug` を正しく接続する
3. WASM API で拡張情報を渡せるようにする
4. テストを追加して動作を検証する

## 変更対象ファイル

| ファイル | 変更内容 | 規模 |
|---|---|---|
| `src/lib.rs` | `compile_to_whitespace` 系 API に拡張フラグ追加 | 小 |
| `src/bin/nospace20.rs` | `target_extensions` をコンパイラに渡す | 小 |
| `src/bin/whitespace20.rs` | `target_extensions` を VM に渡す | 小 |
| `src/wasm_api.rs` | コンパイル API に拡張フラグ対応 | 小 |
| `tests/code_test.rs` | `ws_self` テストでデバッグ拡張対応 | 中 |
| テストリソース | テストケース追加 | 中 |

## 詳細設計

### 1. `lib.rs` の API 変更

既存 API は後方互換性のために維持し、拡張版を追加:

```rust
/// Whitespace にコンパイル（拡張オプション付き）
pub fn compile_to_whitespace_with_options(
    scope: &Scope,
    debug_ext: bool,
) -> Result<String, String> {
    compiler_ws::compile_with_options(scope, debug_ext)
        .map(|prog| prog.to_whitespace())
        .map_err(|e| e.to_string())
}

/// Whitespace にコンパイル（デバッグ用ニーモニック、拡張オプション付き）
pub fn compile_to_whitespace_debug_with_options(
    scope: &Scope,
    debug_ext: bool,
) -> Result<String, String> {
    compiler_ws::compile_with_options(scope, debug_ext)
        .map(|prog| prog.to_debug_string())
        .map_err(|e| e.to_string())
}
```

既存の `compile_to_whitespace(&Scope)` は `compile_to_whitespace_with_options(scope, false)` のラッパーとして維持。

### 2. `nospace20.rs` の変更

```rust
// コンパイルモードでの処理
let debug_ext = property.target_extensions.contains(&TargetExtension::Debug);
let compiled = match property.target {
    CompileTarget::Ws => compile_to_whitespace_with_options(&scope, debug_ext),
    CompileTarget::Mnemonic => compile_to_whitespace_debug_with_options(&scope, debug_ext),
    // ...
};
```

### 3. `whitespace20.rs` の変更

[vm-changes.md](vm-changes.md) に記載済み。`_target_extensions` を使用に変更し、`vm.with_debug_ext(debug_ext)` を適用。

### 4. `wasm_api.rs` の変更

WASM API のコンパイル関数に `target_extensions` パラメータを追加:

```rust
// compile 関数内
CompileTarget::Ws => compile_to_whitespace_with_options(&scope, debug_ext),
CompileTarget::Mnemonic => compile_to_whitespace_debug_with_options(&scope, debug_ext),
```

WASM の `NospaceSession` にも `WhitespaceVM::with_debug_ext` を適用。
具体的な WASM API のインターフェース変更は、既存の WASM ユーザー影響を考慮して慎重に検討する。

## テスト計画

### Unit テスト

#### `compiler_ws/expression.rs`

デバッグ組み込み関数のコード生成テスト（追加検討）:
- `debug_ext=false` 時: 従来通り noop
- `debug_ext=true` 時: 負ヒープアドレスへの Store 命令が生成されること

#### `whitespace/interpreter.rs`

VM の `debug_ext` フラグテスト:
- `debug_ext=false` 時: 負アドレスへの Store が通常ヒープ書き込みになること
- `debug_ext=true` 時: 負アドレスへの Store が拡張 API として処理されること

### Large テスト (統合テスト)

#### 新テストタイプ: `ws_self_debug` / `ws_self_debug_trace`

trace の検証付き ws_self テスト。以下のフローを実行:

1. nospace ソースを `compile_to_whitespace_with_options(scope, true)` でコンパイル
2. `WhitespaceVM::from_source(&ws_code).with_debug_ext(true)` で実行
3. `vm.traced` を期待値と比較

`code_test.rs` に以下のヘルパー関数を追加:

```rust
fn test_whitespace_self_debug_base(test_name: &str) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns").unwrap();
    let check_json_value: serde_json::Value = /* ... */;

    // debug_ext=true でコンパイル
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace_with_options(&a, true).unwrap();

    // debug_ext=true で VM 実行
    let mut vm = WhitespaceVM::from_source(&ws_code).unwrap()
        .with_debug_ext(true);
    let result = vm.run(1_000_000);
    assert_eq!(result, StepResult::Complete);

    // trace 検証
    let check_json: TestConfig = serde_json::from_value(check_json_value).unwrap();
    match check_json {
        TestConfig::Success { trace_hit_counts } => {
            for (i, expected) in trace_hit_counts.into_iter().enumerate() {
                let key = i as i64;
                assert_eq!(
                    vm.traced.get(&key).copied().unwrap_or(0),
                    expected,
                    "trace(idx:{}) mismatch", key
                );
            }
        }
        _ => panic!("Expected success test config"),
    }
}
```

#### テストマニフェスト拡張

`test-manifest.yaml` に新しいテストタイプを追加:

```yaml
- name: test_debug_ext_trace_001
  type: ws_self_debug
  comment: "__trace が whitespace で正しく動作する"
```

#### テストケース案

| テスト名 | 内容 |
|---|---|
| `d0-00-trace-basic` | 基本的な `__trace` の動作確認 |
| `d0-01-assert-pass` | `__assert(1)` が正常に通過すること |
| `d0-02-assert-not-pass` | `__assert_not(0)` が正常に通過すること |
| `d0-03-assert-fail` | `__assert(0)` がエラーになること（runtime error テスト） |
| `d0-04-assert-not-fail` | `__assert_not(1)` がエラーになること |
| `d0-05-trace-multi` | 複数の `__trace` が正しくカウントされること |
| `d0-06-debug-noop-without-ext` | `debug_ext=false` 時に `__trace`/`__assert` が noop になること |

### 既存テストへの影響

- **`test_whitespace_self_base`**: 影響なし。`debug_ext=false` でコンパイル → noop のまま。VM 側も `debug_ext=false` がデフォルトなので通常ヒープ書き込みになるが、noop コンパイルのため負アドレス Store は生成されない。
- **`test_whitespace_self_io_base`**: 同上。影響なし。
- **既存の `ws_self` テスト**: 影響なし。

## spec-whitespace.md の修正

API テーブルのアドレスを修正:

| 修正前 | 修正後 |
|---|---|
| `-10` | `-1` |
| `-11` | `-2` |
| `-12` | `-3` |

または、コードを `-10`/`-11`/`-12` に変更する。いずれかに統一が必要。
コード実装の変更範囲が小さいため、仕様書テーブルの修正を推奨。

## 作業見積もり

| Phase | 変更ファイル数 | コード行数（推定） | 難易度 |
|---|---|---|---|
| Phase 1 (コンパイラ) | 4 | ~80行 | 低 |
| Phase 2 (VM) | 2 | ~30行 | 低 |
| Phase 3 (API・テスト) | 4-6 | ~150行 | 中 |
| 仕様書修正 | 1 | ~10行 | 低 |
| **合計** | **~12** | **~270行** | **低〜中** |
