
# Whitespace VM プロファイラ

Whitespace VM の実行パフォーマンスを計測・比較するためのツール群。

## ツール一覧

| ツール | 説明 |
|--------|------|
| `examples/ws_profiler.rs` | nospace → Whitespace コンパイル＆プロファイル実行（YAML/JSON 出力） |
| `tools/profile-report.py` | プロファイル結果（JSON）から HTML レポートを生成 |

## ws_profiler

nospace ソースコードを Whitespace にコンパイルし、プロファイリングモードで VM を実行して統計情報を出力する。

### 基本的な使い方

```bash
# デフォルトのテストケース (profile-targets.yaml) をプロファイル（YAML 出力）
cargo run --example ws_profiler

# 特定の .ns ファイルを指定
cargo run --example ws_profiler -- path/to/file.ns

# 複数ファイルを指定
cargo run --example ws_profiler -- file1.ns file2.ns
```

### JSON 出力

`--json` フラグを指定すると、YAML の代わりに JSON 形式で出力する。
`tools/profile-report.py` で読み込むにはこの形式が必要。

```bash
# JSON 形式で出力
cargo run --example ws_profiler -- --json

# JSON をファイルに保存
cargo run --example ws_profiler -- --json > tmp/profile.json

# 特定ファイル + JSON
cargo run --example ws_profiler -- --json path/to/file.ns
```

### 出力内容

各テストケースについて以下の情報が出力される:

| フィールド | 説明 |
|------------|------|
| `name` | テストケース名 |
| `source` | ソースファイルパス |
| `compile_success` | コンパイル成功したか |
| `execution.result` | 実行結果 (Complete, Suspended, Error, WaitingForInput) |
| `execution.total_steps` | 実行した総ステップ数 |
| `execution.instruction_counts` | 命令別の実行回数 |
| `execution.memory` | ヒープアクセス統計 |
| `execution.stack` | スタック深さ統計 |
| `execution.program` | 静的命令数、Whitespace テキストサイズ |

### profile-targets.yaml

デフォルト（ファイル未指定）で使われるプロファイル対象は `resources/tests/profile-targets.yaml` に定義されている。

## profile-report.py

`ws_profiler --json` の出力（JSON）を 1 つ以上受け取り、スタンドアロンの HTML レポートを生成する Python スクリプト。Python 3.8 以上、外部パッケージ不要。

### 単体レポート

1 つの JSON ファイルからサマリテーブル + 各テストケースの詳細を生成する。

```bash
# JSON を生成
cargo run --example ws_profiler -- --json > tmp/profile.json

# HTML レポートを生成
python3 tools/profile-report.py tmp/profile.json -o tmp/report.html
```

### 比較レポート

2 つ以上の JSON ファイルを並べて比較する。テストケース名でマッチングし、差分・変化率を色分け表示する。

```bash
# 2つの結果を比較
python3 tools/profile-report.py tmp/before.json tmp/after.json -o tmp/compare.html
```

### ラベル付き比較

`--label` オプションで直後の入力ファイルにラベルを付与できる。未指定時はファイル名がラベルになる。

```bash
python3 tools/profile-report.py \
  --label "v1.0" tmp/profile-v1.json \
  --label "v2.0" tmp/profile-v2.json \
  --label "v3.0" tmp/profile-v3.json \
  -o tmp/compare.html
```

### コマンドライン引数

```
usage: profile-report.py [-h] [-o OUTPUT] [--label LABEL] input [input ...]

positional arguments:
  input          プロファイル JSON ファイル（1つ以上）

optional arguments:
  -h, --help     ヘルプ表示
  -o OUTPUT      出力 HTML ファイルパス（デフォルト: stdout）
  --label LABEL  直後の入力ファイルに付与するラベル（未指定時はファイル名）
```

### HTML レポートの機能

- **サマリテーブル**: ヘッダクリックでカラムソート（昇順/降順トグル）
- **詳細セクション**: テストケース名クリックで展開/折りたたみ
- **命令棒グラフ**: 命令別実行回数の棒グラフ表示
- **差分表示**（比較モード）: 改善（値が減少）を緑、悪化（値が増加）を赤で色分け
- **スタンドアロン**: 外部 CSS/JS 依存なし、単一 HTML ファイルで完結

## 典型的なワークフロー

```bash
# 1. 変更前のプロファイルを取得
cargo run --example ws_profiler -- --json > tmp/profile-before.json

# 2. コードを変更...

# 3. 変更後のプロファイルを取得
cargo run --example ws_profiler -- --json > tmp/profile-after.json

# 4. 比較レポートを生成
python3 tools/profile-report.py \
  --label "before" tmp/profile-before.json \
  --label "after" tmp/profile-after.json \
  -o tmp/profile-compare.html

# 5. ブラウザで確認
open tmp/profile-compare.html
```
