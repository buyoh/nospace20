# Phase 2: CLI に `--strict-heap` オプションを追加

## 対象ファイル

| ファイル | 変更内容 | 規模 |
|---------|---------|------|
| `src/bin/whitespace20.rs` | `--strict-heap` CLI 引数追加、VM への接続 | 小 |
| `src/bin/nospace20.rs` | （変更不要：nospace20 は VM を直接使わない。`--mode run` は nospace インタプリタ、`--mode compile` はコード出力のみ） | - |

## 設計

### `whitespace20` の CLI 引数追加

```rust
#[derive(Parser, Debug)]
struct Args {
    // ... 既存フィールド ...

    /// Treat uninitialized heap access as an error (like wsc default behavior)
    #[arg(long)]
    strict_heap: bool,
}
```

### VM 接続

```rust
fn main() {
    let args = Args::parse();
    // ...
    let mut vm = match WhitespaceVM::from_source(&source) {
        Ok(vm) => vm
            .with_debug_ext(debug_ext)
            .with_strict_heap(args.strict_heap),
        // ...
    };
    // ...
}
```

### nospace20 について

`nospace20 --mode compile` は Whitespace コードを出力するだけであり、VM を使わないため `--strict-heap` は不要。Whitespace コードを実行するのは `whitespace20` の役割。

## 更新履歴

- 2026-02-18: 初版作成
