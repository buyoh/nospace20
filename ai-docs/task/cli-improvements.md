# CLI 改善タスク

## 概要

コマンドラインインターフェースの利便性を向上させるため、以下の機能を追加する。

## 背景

現在の CLI (`src/bin/nospace20.rs`) は標準入力からのみソースコードを読み取る。
デバッグ作業において以下の問題がある:

1. ファイルを実行するのに `cat file.ns | cargo run` が必要
2. `__trace` の結果を確認する方法がない（テストフレームワーク経由のみ）

## 追加機能

### 1. ファイル引数からの読み込み

**優先度**: 高

**現状**:
```bash
cat program.ns | cargo run
# または
cargo run < program.ns
```

**改善後**:
```bash
cargo run -- program.ns
# または
nospace20 program.ns
```

**仕様**:
- 引数がある場合、最初の引数をファイルパスとして解釈
- 引数がない場合、従来通り標準入力から読み取り
- ファイルが存在しない場合はエラーメッセージを表示して終了

### 2. デバッグフラグ (`--debug`)

**優先度**: 高

**用途**: `__trace` の結果を実行完了時に表示

**使用例**:
```bash
nospace20 --debug program.ns
```

**出力例**:
```
main returns: 0

=== Trace Results ===
trace[0]: 3
trace[1]: 2
trace[5]: 1
```

**仕様**:
- `--debug` または `-d` フラグで有効化
- 実行完了後に `Environment.traced` の内容を表示
- trace が空の場合は何も表示しない

### 3. ヘルプ表示 (`--help`)

**優先度**: 中

**使用例**:
```bash
nospace20 --help
```

**出力例**:
```
nospace20 - A nospace language interpreter

USAGE:
    nospace20 [OPTIONS] [FILE]

ARGS:
    <FILE>    Source file to execute (reads from stdin if not provided)

OPTIONS:
    -d, --debug    Show trace results after execution
    -h, --help     Print help information
    -V, --version  Print version information
```

### 4. バージョン表示 (`--version`)

**優先度**: 低

**使用例**:
```bash
nospace20 --version
```

**出力例**:
```
nospace20 0.1.0
```

## 実装方針

### 変更ファイル

- `src/bin/nospace20.rs` のみ

### 依存関係

2つの選択肢:

1. **手動パース** (依存なし)
   - シンプルで軽量
   - 基本的なフラグのみで十分な場合に適切

2. **clap ライブラリ** (外部依存)
   - 機能が豊富
   - ヘルプ自動生成
   - 将来の拡張が容易

**推奨**: 現時点では手動パースで実装。将来的に機能が増えた場合は clap への移行を検討。

### 実装手順

1. [ ] 引数パース処理を追加
2. [ ] ファイル読み込み機能を実装
3. [ ] `--debug` フラグを実装
4. [ ] `--help` フラグを実装
5. [ ] `--version` フラグを実装
6. [ ] テスト・動作確認
7. [ ] README.md にCLI使用法を追記

## 進捗

- [ ] 設計完了
- [ ] 実装完了
- [ ] テスト完了
