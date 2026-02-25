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

プロファイル対象は `resources/tests/profile-targets.yaml` に YAML で定義する。ハードコードではなく外部ファイルとすることで、テストケースの追加・変更が容易になる。

### profile-targets.yaml の形式

```yaml
# Whitespace VM プロファイル対象テストケース
# path: resources/tests/passes/ からの相対パス（拡張子なし）
# comment: (オプション) テストの説明
# stdin: (オプション) stdin に渡す文字列。未指定時は check.json から取得、それもなければ空文字列

targets:
  # 基本
  - path: c000
    comment: "Legacy test - basic functionality"
  - path: c001
    comment: "Legacy test"

  # Examples（complexity escalation）
  - path: examples/e0-00-puts
    comment: "puts example"
  - path: examples/e0-01-fibonacci
    comment: "Fibonacci example"
  - path: examples/e1-00-qsort
    comment: "Quicksort example"
    stdin: "5\n3 1 4 1 5\n"
  - path: examples/e1-01-queue
    comment: "Queue example"
    stdin: "5\n1 2 3 4 5\n"

  # 配列操作
  - path: array-basic
    comment: "Basic array operations"
  - path: array-static
    comment: "Static array"
  - path: array-reference
    comment: "Array reference"

  # 文字列
  - path: string-basic
    comment: "Basic string operations"

  # 制御フロー
  - path: control_flow/if_001
    comment: "If statement"
  - path: control_flow/while_001
    comment: "While loop"

  # 関数
  - path: functions/func_001
    comment: "Basic function"
  - path: functions/func_recursive_001
    comment: "Recursive function"

  # 統合
  - path: integration/integ_001
    comment: "Integration test"
```

### YAML 読み込み用構造体

```rust
#[derive(Deserialize)]
struct ProfileTargets {
    targets: Vec<ProfileTarget>,
}

#[derive(Deserialize)]
struct ProfileTarget {
    path: String,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    stdin: Option<String>,
}
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

const PROFILE_TARGETS_PATH: &str = "resources/tests/profile-targets.yaml";

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let targets = if args.len() > 1 {
        // 引数で指定されたファイル（path として扱う）
        args[1..].iter().map(|s| ProfileTarget {
            path: s.clone(),
            comment: None,
            stdin: None,
        }).collect()
    } else {
        // YAML からデフォルトのテストケースを読み込み
        let yaml_content = fs::read_to_string(PROFILE_TARGETS_PATH)
            .expect("Failed to read profile-targets.yaml");
        let manifest: ProfileTargets = serde_yaml::from_str(&yaml_content)
            .expect("Failed to parse profile-targets.yaml");
        manifest.targets
    };

    let mut profiles = Vec::new();
    for target in &targets {
        let profile = run_profile(target);
        profiles.push(profile);
    }

    // YAML 出力
    let output = serde_yaml::to_string(&ProfileReport { profiles }).unwrap();
    println!("{}", output);
}

fn run_profile(target: &ProfileTarget) -> ProfileEntry {
    // 1. .ns ファイルを読み込み（resources/tests/passes/{path}.ns）
    // 2. parse → compile_to_whitespace_with_options
    // 3. WhitespaceVM::from_source + with_profiling(true) + with_debug_ext(true)
    // 4. stdin 設定: target.stdin > check.json の stdin/stdin_file > 空文字列
    // 5. vm.run(10_000_000)
    // 6. ProfileStats から ProfileEntry を構築
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
