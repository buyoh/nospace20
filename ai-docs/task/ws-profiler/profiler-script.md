# プロファイラスクリプト設計

## 概要

`examples/ws_profiler.rs` として Rust スクリプトを作成する。指定されたテストケース（デフォルトで `resources/tests/passes/` の代表的なケース）を nospace → Whitespace にコンパイルし、プロファイリングモードで実行して統計を YAML で出力する。

## 実行方法

```bash
# デフォルトのテストケースをプロファイル
cargo run --example ws_profiler

# 特定の .ns ファイルを指定
cargo run --example ws_profiler -- path/to/file.ns

# 出力をファイルに保存
cargo run --example ws_profiler > profile-output.yaml
```

## 出力形式（YAML）

```yaml
# Whitespace VM Profile Report
profiles:
  - name: "examples/e0-00-puts"
    source: "resources/tests/passes/examples/e0-00-puts.ns"
    compile_success: true
    execution:
      result: "Complete"  # Complete | Suspended | Error
      total_steps: 1234
      instruction_counts:
        push: 100
        duplicate: 20
        copy: 5
        swap: 10
        discard: 15
        add: 50
        sub: 30
        mul: 10
        div: 5
        modulo: 2
        store: 80
        retrieve: 60
        label: 40
        call: 25
        jump: 30
        jump_if_zero: 20
        jump_if_negative: 10
        return: 25
        exit: 1
        output_char: 12
        output_number: 0
        input_char: 0
        input_number: 0
      memory:
        heap_store_range: [0, 255]    # [min, max] or null
        heap_retrieve_range: [0, 200]
        heap_store_count: 80
        heap_retrieve_count: 60
        heap_unique_addresses: 50
      stack:
        max_data_stack_depth: 15
        max_call_stack_depth: 8
      program:
        instruction_count: 350   # コンパイル後の静的命令数
        whitespace_size: 4500    # Whitespace テキストのバイト数

  - name: "examples/e1-00-qsort"
    # ...
```

## デフォルトのテストケース

以下のカテゴリから代表的なケースをピックアップ:

```rust
const DEFAULT_TEST_CASES: &[&str] = &[
    // 基本
    "c000",
    "c001",
    // Examples（complexity escalation）
    "examples/e0-00-puts",
    "examples/e0-01-fibonacci",
    "examples/e1-00-qsort",
    "examples/e1-01-queue",
    // 配列操作
    "array-basic",
    "array-static",
    "array-reference",
    // 文字列
    "string-basic",
    // 制御フロー
    "control_flow/if_001",
    "control_flow/while_001",
    // 関数
    "functions/func_001",
    "functions/func_recursive_001",
    // 統合
    "integration/integ_001",
];
```

## 実装

### 依存関係

`Cargo.toml` に追加:

```toml
[dev-dependencies]
serde_yaml = "0.9"

[[example]]
name = "ws_profiler"
required-features = ["cli"]
```

注意: `serde_yaml` は既に `[build-dependencies]` にあるので、`[dev-dependencies]` にも追加する。

### プログラム構造

```rust
// examples/ws_profiler.rs

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let test_cases = if args.len() > 1 {
        // 引数で指定されたファイル
        args[1..].to_vec()
    } else {
        // デフォルトのテストケース
        DEFAULT_TEST_CASES.iter().map(|s| s.to_string()).collect()
    };

    let mut profiles = Vec::new();
    for case in &test_cases {
        let profile = run_profile(case);
        profiles.push(profile);
    }

    // YAML 出力
    let output = serde_yaml::to_string(&ProfileReport { profiles }).unwrap();
    println!("{}", output);
}

fn run_profile(test_case: &str) -> ProfileEntry {
    // 1. .ns ファイルを読み込み
    // 2. parse → compile_to_whitespace_with_options
    // 3. WhitespaceVM::from_source + with_profiling(true) + with_debug_ext(true)
    // 4. vm.run(10_000_000)
    // 5. ProfileStats から ProfileEntry を構築
}
```

### Serde 出力用構造体

```rust
#[derive(Serialize)]
struct ProfileReport {
    profiles: Vec<ProfileEntry>,
}

#[derive(Serialize)]
struct ProfileEntry {
    name: String,
    source: String,
    compile_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution: Option<ExecutionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct ExecutionProfile {
    result: String,
    total_steps: usize,
    instruction_counts: InstructionCountsYaml,
    memory: MemoryProfile,
    stack: StackProfile,
    program: ProgramProfile,
}
```

## 最大ステップ数

デフォルトで 10,000,000 ステップをリミットとする。Suspended の場合はその時点の統計を出力し、`result: "Suspended"` とする。

## I/O の扱い

- stdin: テストケースに `check.json` の `stdin` / `stdin_file` がある場合はそれを使用。なければ空文字列。
- stdout: `Vec<u8>` に書き込み（出力は破棄）。
- `debug_ext: true` で実行（`__trace` 等の拡張 API をフック）。
