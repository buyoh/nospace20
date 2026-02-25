# HTML レポート生成スクリプト設計

## 概要

`tools/profile-report.py` として Python スクリプトを作成する。`ws_profiler --json` の出力（JSON）を1つ以上受け取り、比較可能な HTML レポートを生成する。

## 使い方

```bash
# 1つのプロファイル結果からサマリレポート
cargo run --example ws_profiler -- --json > tmp/profile-latest.json
python3 tools/profile-report.py tmp/profile-latest.json -o tmp/profile-report.html

# 2つの結果を比較
python3 tools/profile-report.py tmp/profile-before.json tmp/profile-after.json -o tmp/profile-compare.html

# 3つ以上の比較も可能（ラベル付き）
python3 tools/profile-report.py \
  --label "v1.0" tmp/profile-v1.json \
  --label "v2.0" tmp/profile-v2.json \
  --label "v3.0" tmp/profile-v3.json \
  -o tmp/profile-compare.html
```

## コマンドライン引数

```
usage: profile-report.py [-h] [-o OUTPUT] [--label LABEL] input [input ...]

positional arguments:
  input          プロファイル JSON ファイル（1つ以上）

optional arguments:
  -h, --help     ヘルプ表示
  -o OUTPUT      出力 HTML ファイルパス（デフォルト: stdout）
  --label LABEL  直後の入力ファイルに付与するラベル（未指定時はファイル名）
```

### 引数パース

`argparse` は `--label` と positional args の交互指定が難しいので、手動パースする。

```python
def parse_args(argv):
    """
    引数を解析して、(inputs: list[(label, path)], output: str|None) を返す。
    --label LABEL は直後の positional arg にバインドされる。
    """
    inputs = []
    output = None
    pending_label = None
    i = 0
    while i < len(argv):
        arg = argv[i]
        if arg in ('-h', '--help'):
            print_usage()
            sys.exit(0)
        elif arg in ('-o', '--output'):
            i += 1
            output = argv[i]
        elif arg == '--label':
            i += 1
            pending_label = argv[i]
        else:
            label = pending_label if pending_label else os.path.basename(arg)
            inputs.append((label, arg))
            pending_label = None
        i += 1
    return inputs, output
```

## HTML レポート構造

### 単体レポート（入力1ファイル）

```
┌─────────────────────────────────────────────┐
│  Whitespace VM Profile Report               │
│  Generated: 2026-02-25 12:00:00             │
├─────────────────────────────────────────────┤
│  Summary Table                               │
│  ┌────────────┬──────┬───────┬──────┬─────┐ │
│  │ Name       │Steps │Instr. │Heap  │Stack│ │
│  ├────────────┼──────┼───────┼──────┼─────┤ │
│  │ c000       │   50 │   86  │    2 │   6 │ │
│  │ c001       │  228 │  175  │    5 │   8 │ │
│  │ ...        │  ... │  ...  │  ... │ ... │ │
│  └────────────┴──────┴───────┴──────┴─────┘ │
├─────────────────────────────────────────────┤
│  Detail: c000                                │
│  - Execution result: Complete                │
│  - Instruction breakdown (bar chart)         │
│  - Memory stats                              │
│  - Stack stats                               │
│  - Program stats                             │
├─────────────────────────────────────────────┤
│  Detail: c001                                │
│  ...                                         │
└─────────────────────────────────────────────┘
```

### 比較レポート（入力2ファイル以上）

```
┌─────────────────────────────────────────────────────────┐
│  Whitespace VM Profile Comparison                        │
│  Sources: profile-before.json vs profile-after.json      │
├─────────────────────────────────────────────────────────┤
│  Comparison Table                                        │
│  ┌────────────┬──────────┬──────────┬──────────────────┐ │
│  │ Name       │ Before   │ After    │ Diff             │ │
│  ├────────────┼──────────┼──────────┼──────────────────┤ │
│  │ c000       │       50 │       48 │    -2 (-4.0%) ▼  │ │
│  │ c001       │      228 │      250 │   +22 (+9.6%) ▲  │ │
│  └────────────┴──────────┴──────────┴──────────────────┘ │
│                                                          │
│  Per-metric comparison tables:                           │
│  - Total Steps                                           │
│  - Instruction Count (static)                            │
│  - Whitespace Size                                       │
│  - Heap Unique Addresses                                 │
│  - Max Data Stack Depth                                  │
│  - Max Call Stack Depth                                   │
├─────────────────────────────────────────────────────────┤
│  Detail per test case (expandable)                       │
│  - Side-by-side instruction breakdown                    │
└─────────────────────────────────────────────────────────┘
```

## HTML 生成方針

### スタンドアロン HTML

- CSS はすべて `<style>` タグで埋め込み
- JavaScript はすべて `<script>` タグで埋め込み
- 外部リソースへの依存なし
- `<!DOCTYPE html>` + UTF-8

### CSS スタイル

- シンプルなテーブルスタイル（枠線、ストライプ行）
- 差分の色分け: 改善（値が減少）= 緑、悪化（値が増加）= 赤
- レスポンシブではないが、テーブルは横スクロール可能
- ダークモード未対応（シンプルさ優先）

### JavaScript 機能

最小限のインタラクティブ機能:

- **テーブルソート**: ヘッダクリックでカラムソート（昇順/降順トグル）
- **詳細展開**: テストケース名をクリックで詳細セクションを展開/折りたたみ
- フレームワーク不使用、vanilla JavaScript のみ

## データ処理

### JSON 読み込み

```python
import json

def load_profile(path):
    with open(path) as f:
        data = json.load(f)
    return data  # {"profiles": [...]}
```

### メトリクス抽出

サマリテーブルに表示する主要メトリクス:

| カラム | フィールド | 説明 |
|---|---|---|
| Name | `name` | テストケース名 |
| Result | `execution.result` | 実行結果 |
| Steps | `execution.total_steps` | 総ステップ数 |
| Instructions | `execution.program.instruction_count` | 静的命令数 |
| WS Size | `execution.program.whitespace_size` | WS テキストサイズ |
| Heap Addrs | `execution.memory.heap_unique_addresses` | ユニークヒープアドレス数 |
| Max Stack | `execution.stack.max_data_stack_depth` | データスタック最大深度 |
| Max Call | `execution.stack.max_call_stack_depth` | コールスタック最大深度 |

### 比較計算

```python
def compute_diff(before, after):
    """数値の差分と変化率を計算"""
    if before is None or after is None:
        return None
    diff = after - before
    pct = (diff / before * 100) if before != 0 else float('inf') if diff != 0 else 0
    return {"before": before, "after": after, "diff": diff, "pct": pct}
```

### テストケースのマッチング

比較時、テストケース名（`name` フィールド）をキーとしてマッチングする。
- 両方に存在: 比較表示
- 片方のみ: 「追加」「削除」として表示

## テンプレート方式

Python の `string.Template` や f-string を使用してHTML文字列を構築する。Jinja2 等の外部テンプレートエンジンは使用しない。

```python
def generate_html(inputs, is_comparison):
    """
    inputs: list[(label, data)]  data は load_profile() の返り値
    is_comparison: bool
    """
    parts = []
    parts.append(HTML_HEADER)
    if is_comparison:
        parts.append(generate_comparison_tables(inputs))
    else:
        parts.append(generate_summary_table(inputs[0]))
    parts.append(generate_detail_sections(inputs))
    parts.append(HTML_FOOTER)
    return "\n".join(parts)
```

## 配置

- `tools/profile-report.py`: メインスクリプト
- 出力先のデフォルト: stdout（`-o` で指定可能）
- 生成結果の推奨保存先: `tmp/` ディレクトリ（.gitignore 済み）

## 制約・前提

- Python 3.8 以上（f-string、`:=` は使用しない）
- 外部パッケージ不要（`json`, `html`, `os`, `sys`, `datetime` のみ使用）
- 入力は `ws_profiler --json` の出力形式に準拠する JSON ファイル
- 出力は UTF-8 エンコーディングの HTML5 ドキュメント
