# Phase 3: テスト基盤に strict-heap テスト実行を追加

## 概要

nospace → Whitespace コンパイル → 組み込み VM 実行のテスト（`whitespace-self` ターゲット）に strict-heap モードのバリアントを追加する。

## 対象ファイル

| ファイル | 変更内容 | 規模 |
|---------|---------|------|
| `build.rs` | strict-heap テストバリアントの自動生成 | 中 |
| `tests/code_test.rs` | `test_whitespace_self_base` 系に strict_heap パラメータ追加 | 小 |
| `resources/tests/test-manifest.yaml` | `exclude_targets` に `whitespace-self-strict` を追加可能に | 小 |

## 設計

### 新しいテストターゲット: `whitespace-self-strict`

`whitespace-self` テストに加え、strict-heap 有効での実行バリアントを追加する。

- ターゲット名: `whitespace-self-strict`
- `exclude_targets` で除外可能
- デフォルトで全 `success` / `success_io` テストに生成

### build.rs の変更

`exclude_targets` に `whitespace-self-strict` を追加し、除外されていない場合は strict-heap テストを生成する。

```rust
let has_whitespace_self_strict = !exclude_targets.iter().any(|t| t == "whitespace-self-strict");
```

生成するテスト（`success` の場合）:

```rust
// strict-heap バリアント
#[test]
fn {name}_ws_self_strict() {{
    test_whitespace_self_base_strict("{path}", {debug_ext})
}}
```

`success_io` の場合も同様に `test_whitespace_self_io_base_strict` を呼び出す。

### tests/code_test.rs の変更

新しいヘルパー関数を追加:

```rust
fn test_whitespace_self_base_strict(test_name: &str, debug_ext: bool) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    // コンパイル
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace_with_options(&a, debug_ext)
        .unwrap_or_else(|e| panic!("Compilation failed: {}", e));

    // 独自 WhitespaceVM で実行（strict-heap 有効）
    let mut vm = WhitespaceVM::from_source(&ws_code)
        .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e))
        .with_debug_ext(debug_ext)
        .with_strict_heap(true);

    let result = vm.run(1_000_000);

    match result {
        StepResult::Complete => {}
        StepResult::Suspended => panic!(
            "Whitespace execution suspended (exceeded step limit) for {}",
            test_name
        ),
        StepResult::Error(e) => panic!(
            "Whitespace execution failed for {} (strict-heap): {:?}",
            test_name, e
        ),
    }
}

fn test_whitespace_self_io_base_strict(test_name: &str, debug_ext: bool) {
    // test_whitespace_self_io_base_debug と同じだが with_strict_heap(true) を追加
    // ...
}
```

### テストマニフェストでの除外

strict-heap でエラーが発生することが分かっているテスト（例えばユーザ変数の初期化忘れがある意図的なテスト）は `exclude_targets` で除外可能:

```yaml
- name: test_some_test
  type: success
  path: some/test
  exclude_targets: [whitespace-self-strict]
```

### strict-heap テストが失敗する可能性のあるケース

nospace コンパイラが正しく変数を初期化していれば strict-heap テストは全て成功するはずだが、以下のケースで失敗する可能性がある:

1. **ローカル変数の未初期化**: `generate_local_allocate` はヒープポインタを進めるが、確保した領域のゼロクリアは行わない。var 宣言で初期値なしの場合、`retrieve` で未初期化ヒープアクセスが発生する
2. **予約アドレスの初期化**: `LOCAL_HEAP_BEGIN`(2), `LOCAL_HEAP_END`(3) 等の予約アドレスはプログラム先頭で初期化されているか確認が必要
3. **グローバル変数の初期化**: グローバル変数領域のゼロクリアが必要

#### 対処方針

strict-heap テストを実装後、失敗するテストを確認し、必要に応じて:
- コンパイラにゼロクリアコードを追加（`generate_local_allocate` でのゼロフィル等）
- または該当テストを `exclude_targets: [whitespace-self-strict]` で除外

**ゼロクリアが必要な場合は別タスクとして対応する。** Phase 3 ではテスト基盤の整備のみを行い、失敗するテストは除外する。

## 実装順序

1. `tests/code_test.rs` にヘルパー関数追加
2. `build.rs` のテスト生成ロジック更新
3. ビルド・テスト実行
4. 失敗するテストを `exclude_targets: [whitespace-self-strict]` で除外

## 更新履歴

- 2026-02-18: 初版作成
