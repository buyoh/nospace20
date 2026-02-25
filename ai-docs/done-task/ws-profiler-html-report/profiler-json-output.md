# ws_profiler JSON 出力追加

## 概要

`examples/ws_profiler.rs` に JSON 出力モードを追加する。HTML レポートスクリプトが Python 標準ライブラリのみで読み込めるようにする。

## 動機

- 現在の出力は YAML のみ
- Python で YAML を読むには `PyYAML`（外部パッケージ）が必要
- JSON なら Python 標準ライブラリの `json` モジュールで読める
- JSON は比較ツール等との連携も容易

## 設計

### コマンドライン引数

```bash
# デフォルト: YAML 出力（既存動作維持）
cargo run --example ws_profiler

# JSON 出力
cargo run --example ws_profiler -- --json

# JSON + 特定ファイル指定
cargo run --example ws_profiler -- --json path/to/file.ns

# ファイルに保存
cargo run --example ws_profiler -- --json > profile-output.json
```

### 引数パースの実装

外部クレート不要。`std::env::args()` を手動パースする。

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut json_output = false;
    let mut paths: Vec<String> = Vec::new();

    for arg in &args[1..] {
        match arg.as_str() {
            "--json" => json_output = true,
            _ => paths.push(arg.clone()),
        }
    }

    // ... プロファイル実行 ...

    if json_output {
        let json = serde_json::to_string_pretty(&report).unwrap();
        println!("{}", json);
    } else {
        // 既存の YAML 出力
        println!("# Whitespace VM Profile Report");
        let yaml = serde_yaml::to_string(&report).unwrap();
        print!("{}", yaml);
    }
}
```

### JSON 出力形式

既存の YAML 出力構造をそのまま JSON にシリアライズする。`serde_json` は既に依存にあるため追加不要。

```json
{
  "profiles": [
    {
      "name": "c000",
      "source": "resources/tests/passes/c000.ns",
      "compile_success": true,
      "execution": {
        "result": "Complete",
        "total_steps": 50,
        "instruction_counts": {
          "push": 15,
          "duplicate": 1,
          "copy": 2,
          "swap": 6,
          "discard": 2,
          "add": 1,
          "sub": 0,
          "mul": 0,
          "div": 0,
          "modulo": 0,
          "store": 7,
          "retrieve": 3,
          "label": 5,
          "call": 3,
          "jump": 2,
          "jump_if_zero": 0,
          "jump_if_negative": 0,
          "return": 3,
          "exit": 1,
          "output_char": 0,
          "output_number": 0,
          "input_char": 0,
          "input_number": 0
        },
        "memory": {
          "heap_store_range": [2, 3],
          "heap_retrieve_range": [2, 3],
          "heap_store_count": 6,
          "heap_retrieve_count": 3,
          "heap_unique_addresses": 2
        },
        "stack": {
          "max_data_stack_depth": 6,
          "max_call_stack_depth": 2
        },
        "program": {
          "instruction_count": 86,
          "whitespace_size": 457
        }
      }
    }
  ]
}
```

### 依存関係

- `serde_json`: 既に `[dependencies]` に含まれているため追加不要
- `serde_yaml`: 既存のまま

### 変更対象ファイル

- `examples/ws_profiler.rs`: `main()` の引数パースと出力分岐を変更

## `--json` 追加時の構造体への影響

既存の `#[derive(Serialize)]` 構造体群はそのまま流用。`serde` が JSON/YAML 両方に対応しているため、追加の構造体定義は不要。
