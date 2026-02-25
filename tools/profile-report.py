#!/usr/bin/env python3
"""
Whitespace VM プロファイル HTML レポート生成スクリプト

ws_profiler --json の出力 (JSON) を 1 つ以上受け取り、
比較可能な HTML レポートを生成する。

使い方:
    # 単体レポート
    python3 tools/profile-report.py tmp/profile.json -o tmp/report.html

    # 2つの結果を比較
    python3 tools/profile-report.py tmp/before.json tmp/after.json -o tmp/compare.html

    # ラベル付き比較
    python3 tools/profile-report.py \\
        --label "v1.0" tmp/v1.json \\
        --label "v2.0" tmp/v2.json \\
        -o tmp/compare.html
"""

import json
import html
import os
import sys
import datetime


# ===== 引数パース =====

def print_usage():
    print("""usage: profile-report.py [-h] [-o OUTPUT] [--label LABEL] input [input ...]

positional arguments:
  input          プロファイル JSON ファイル（1つ以上）

optional arguments:
  -h, --help     ヘルプ表示
  -o OUTPUT      出力 HTML ファイルパス（デフォルト: stdout）
  --label LABEL  直後の入力ファイルに付与するラベル（未指定時はファイル名）
""", file=sys.stderr)


def parse_args(argv):
    """
    引数を解析して (inputs: list[(label, path)], output: str|None) を返す。
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
            if i >= len(argv):
                print("Error: -o requires an argument", file=sys.stderr)
                sys.exit(1)
            output = argv[i]
        elif arg == '--label':
            i += 1
            if i >= len(argv):
                print("Error: --label requires an argument", file=sys.stderr)
                sys.exit(1)
            pending_label = argv[i]
        else:
            label = pending_label if pending_label else os.path.basename(arg)
            inputs.append((label, arg))
            pending_label = None
        i += 1
    return inputs, output


# ===== JSON 読み込み =====

def load_profile(path):
    """JSON ファイルを読み込んで dict を返す。{"profiles": [...]}"""
    with open(path, encoding='utf-8') as f:
        return json.load(f)


# ===== メトリクス抽出 =====

METRICS = [
    ("Steps",        lambda e: e.get("total_steps")),
    ("Instructions", lambda e: e.get("program", {}).get("instruction_count")),
    ("WS Size",      lambda e: e.get("program", {}).get("whitespace_size")),
    ("Heap Addrs",   lambda e: e.get("memory", {}).get("heap_unique_addresses")),
    ("Max Stack",    lambda e: e.get("stack", {}).get("max_data_stack_depth")),
    ("Max Call",     lambda e: e.get("stack", {}).get("max_call_stack_depth")),
]

INSTRUCTION_FIELDS = [
    "push", "duplicate", "copy", "swap", "discard",
    "add", "sub", "mul", "div", "modulo",
    "store", "retrieve",
    "label", "call", "jump", "jump_if_zero", "jump_if_negative",
    "return", "exit",
    "output_char", "output_number", "input_char", "input_number",
]


def get_execution(profile_entry):
    """profile_entry から execution dict を取得する。なければ None。"""
    return profile_entry.get("execution")


def get_metric(profile_entry, metric_fn):
    """profile_entry からメトリクス値を取得する。なければ None。"""
    ex = get_execution(profile_entry)
    if ex is None:
        return None
    return metric_fn(ex)


# ===== 差分計算 =====

def compute_diff(before, after):
    """数値の差分と変化率を計算する。"""
    if before is None or after is None:
        return None
    diff = after - before
    if before != 0:
        pct = diff / before * 100
    elif diff != 0:
        pct = float('inf')
    else:
        pct = 0.0
    return {"before": before, "after": after, "diff": diff, "pct": pct}


def format_diff(diff_info):
    """差分情報を HTML 文字列にフォーマットする（色付き）。"""
    if diff_info is None:
        return "<span>N/A</span>"
    d = diff_info["diff"]
    pct = diff_info["pct"]
    if d == 0:
        return "<span>±0</span>"
    sign = "+" if d > 0 else ""
    if pct == float('inf'):
        pct_str = "+∞%"
    else:
        pct_str = f"{sign}{pct:.1f}%"
    # 増加は悪化（赤）、減少は改善（緑）
    css_class = "worse" if d > 0 else "better"
    arrow = "▲" if d > 0 else "▼"
    return f'<span class="{css_class}">{sign}{d} ({pct_str}) {arrow}</span>'


def esc(s):
    """HTML エスケープ。"""
    return html.escape(str(s))


# ===== HTML 定数 =====

HTML_HEADER = """<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Whitespace VM Profile Report</title>
<style>
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    font-size: 14px;
    background: #f5f5f5;
    color: #333;
    margin: 0;
    padding: 16px;
  }
  h1 { font-size: 1.4em; margin-bottom: 4px; }
  h2 { font-size: 1.1em; margin-top: 24px; margin-bottom: 8px; border-bottom: 2px solid #ddd; padding-bottom: 4px; }
  h3 { font-size: 1.0em; margin: 12px 0 4px; }
  p.subtitle { color: #666; margin: 0 0 16px; font-size: 0.9em; }
  .table-wrap { overflow-x: auto; }
  table {
    border-collapse: collapse;
    min-width: 100%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  }
  th, td {
    border: 1px solid #ddd;
    padding: 6px 10px;
    text-align: right;
    white-space: nowrap;
  }
  th {
    background: #4a6fa5;
    color: #fff;
    cursor: pointer;
    user-select: none;
    text-align: center;
  }
  th:hover { background: #3a5f95; }
  th.sorted-asc::after { content: " ▲"; }
  th.sorted-desc::after { content: " ▼"; }
  tr:nth-child(even) { background: #f9f9f9; }
  tr:hover { background: #eef3fb; }
  td.name-col { text-align: left; cursor: pointer; color: #2a5db0; }
  td.name-col:hover { text-decoration: underline; }
  td.result-col { text-align: center; }
  .better { color: #1a7a1a; font-weight: bold; }
  .worse  { color: #b02020; font-weight: bold; }
  .error-row td { color: #b02020; font-style: italic; }
  .detail-section {
    background: #fff;
    border: 1px solid #ddd;
    border-radius: 4px;
    margin: 8px 0;
    box-shadow: 0 1px 3px rgba(0,0,0,0.07);
  }
  .detail-header {
    padding: 8px 12px;
    background: #e8edf5;
    cursor: pointer;
    font-weight: bold;
    user-select: none;
    border-radius: 4px 4px 0 0;
  }
  .detail-header:hover { background: #d8e2f0; }
  .detail-body { padding: 12px; display: none; }
  .detail-body.open { display: block; }
  .instr-bar-container { display: flex; align-items: center; gap: 6px; margin: 2px 0; }
  .instr-bar-label { width: 130px; text-align: right; font-size: 0.85em; color: #555; }
  .instr-bar-wrap { flex: 1; background: #eee; border-radius: 3px; overflow: hidden; height: 14px; }
  .instr-bar { background: #4a6fa5; height: 14px; border-radius: 3px; min-width: 0; }
  .instr-bar-value { width: 60px; font-size: 0.85em; color: #333; }
  .two-col { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .stat-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 6px; }
  .stat-item { background: #f2f5fb; border-radius: 4px; padding: 6px 10px; }
  .stat-label { font-size: 0.8em; color: #666; }
  .stat-value { font-size: 1.1em; font-weight: bold; color: #2a5db0; }
  .missing { color: #aaa; font-style: italic; }
  footer { margin-top: 32px; font-size: 0.8em; color: #aaa; }
</style>
</head>
<body>
"""

HTML_FOOTER = """
<footer>Generated by tools/profile-report.py</footer>
<script>
// テーブルソート
function sortTable(table, colIndex) {
  var rows = Array.from(table.querySelectorAll('tbody tr'));
  var th = table.querySelectorAll('thead th')[colIndex];
  var asc = !th.classList.contains('sorted-asc');
  table.querySelectorAll('thead th').forEach(function(h) {
    h.classList.remove('sorted-asc', 'sorted-desc');
  });
  th.classList.add(asc ? 'sorted-asc' : 'sorted-desc');
  rows.sort(function(a, b) {
    var av = a.cells[colIndex] ? a.cells[colIndex].getAttribute('data-v') || a.cells[colIndex].textContent : '';
    var bv = b.cells[colIndex] ? b.cells[colIndex].getAttribute('data-v') || b.cells[colIndex].textContent : '';
    var an = parseFloat(av), bn = parseFloat(bv);
    if (!isNaN(an) && !isNaN(bn)) { return asc ? an - bn : bn - an; }
    return asc ? av.localeCompare(bv) : bv.localeCompare(av);
  });
  var tbody = table.querySelector('tbody');
  rows.forEach(function(r) { tbody.appendChild(r); });
}
document.querySelectorAll('table.sortable').forEach(function(table) {
  table.querySelectorAll('thead th').forEach(function(th, i) {
    th.addEventListener('click', function() { sortTable(table, i); });
  });
});

// 詳細セクション展開
document.querySelectorAll('.detail-header').forEach(function(header) {
  header.addEventListener('click', function() {
    var body = header.nextElementSibling;
    body.classList.toggle('open');
  });
});

// サマリテーブルの名前クリックで詳細展開
document.querySelectorAll('td.name-col[data-detail]').forEach(function(td) {
  td.addEventListener('click', function() {
    var id = td.getAttribute('data-detail');
    var body = document.getElementById(id);
    if (body) {
      body.classList.add('open');
      body.scrollIntoView({behavior: 'smooth', block: 'start'});
    }
  });
});
</script>
</body>
</html>
"""


# ===== 命令棒グラフ生成 =====

def generate_instr_bars(instr_counts, max_val=None):
    """命令別カウントの棒グラフ HTML を生成する。"""
    if not instr_counts:
        return "<p class='missing'>命令カウントなし</p>"
    if max_val is None:
        max_val = max((instr_counts.get(f, 0) for f in INSTRUCTION_FIELDS), default=1)
    if max_val == 0:
        max_val = 1
    lines = []
    for field in INSTRUCTION_FIELDS:
        val = instr_counts.get(field, 0)
        width_pct = int(val / max_val * 100)
        lines.append(
            f'<div class="instr-bar-container">'
            f'<div class="instr-bar-label">{esc(field)}</div>'
            f'<div class="instr-bar-wrap"><div class="instr-bar" style="width:{width_pct}%"></div></div>'
            f'<div class="instr-bar-value">{val}</div>'
            f'</div>'
        )
    return "\n".join(lines)


# ===== スタット表示 =====

def stat_item(label, value):
    if value is None:
        value_html = "<span class='missing'>N/A</span>"
    else:
        value_html = f"<span class='stat-value'>{esc(value)}</span>"
    return f"<div class='stat-item'><div class='stat-label'>{esc(label)}</div>{value_html}</div>"


# ===== 単体詳細セクション =====

def generate_single_detail(entry, idx):
    """単体レポート用の詳細セクション HTML を生成する。"""
    name = entry.get("name", f"entry-{idx}")
    detail_id = f"detail-{idx}"
    ex = get_execution(entry)

    lines = []
    lines.append(f'<div class="detail-section">')
    lines.append(f'<div class="detail-header">▶ {esc(name)}</div>')
    lines.append(f'<div class="detail-body" id="{detail_id}">')

    if ex is None:
        error = entry.get("error", "不明なエラー")
        lines.append(f'<p style="color:#b02020">❌ {esc(error)}</p>')
    else:
        result = ex.get("result", "?")
        lines.append(f'<p><strong>Result:</strong> {esc(result)} &nbsp; <strong>Total Steps:</strong> {ex.get("total_steps", "N/A")}</p>')

        # 統計グリッド
        mem = ex.get("memory", {})
        stk = ex.get("stack", {})
        prog = ex.get("program", {})
        lines.append("<div class='stat-grid'>")
        lines.append(stat_item("Instructions (static)", prog.get("instruction_count")))
        lines.append(stat_item("Whitespace Size", prog.get("whitespace_size")))
        lines.append(stat_item("Heap Unique Addr", mem.get("heap_unique_addresses")))
        lines.append(stat_item("Heap Store Count", mem.get("heap_store_count")))
        lines.append(stat_item("Heap Retrieve Count", mem.get("heap_retrieve_count")))
        sr = mem.get("heap_store_range")
        lines.append(stat_item("Heap Store Range", f"{sr[0]}..{sr[1]}" if sr else None))
        rr = mem.get("heap_retrieve_range")
        lines.append(stat_item("Heap Retrieve Range", f"{rr[0]}..{rr[1]}" if rr else None))
        lines.append(stat_item("Max Data Stack", stk.get("max_data_stack_depth")))
        lines.append(stat_item("Max Call Stack", stk.get("max_call_stack_depth")))
        lines.append("</div>")

        # 命令棒グラフ
        lines.append("<h3>命令別実行回数</h3>")
        ic = ex.get("instruction_counts", {})
        # instruction_counts の値は "return" キーの場合もあるが、ws_profiler では "return" フィールド名で出力
        lines.append(generate_instr_bars(ic))

    lines.append("</div></div>")
    return "\n".join(lines)


# ===== 比較詳細セクション =====

def generate_comparison_detail(name, entries_by_label, idx):
    """比較レポート用の詳細セクション HTML を生成する。"""
    detail_id = f"detail-{idx}"
    labels = list(entries_by_label.keys())

    lines = []
    lines.append(f'<div class="detail-section">')
    lines.append(f'<div class="detail-header">▶ {esc(name)}</div>')
    lines.append(f'<div class="detail-body" id="{detail_id}">')

    # 各ラベルの命令棒グラフを横並びにする
    has_any = False
    for lbl in labels:
        entry = entries_by_label[lbl]
        if entry is None:
            continue
        ex = get_execution(entry)
        if ex and ex.get("instruction_counts"):
            has_any = True
            break

    if has_any:
        lines.append(f'<div class="two-col">')
        for lbl in labels:
            entry = entries_by_label[lbl]
            lines.append(f'<div><h3>{esc(lbl)}</h3>')
            if entry is None:
                lines.append("<p class='missing'>データなし</p>")
            else:
                ex = get_execution(entry)
                if ex:
                    ic = ex.get("instruction_counts", {})
                    lines.append(generate_instr_bars(ic))
                else:
                    error = entry.get("error", "不明なエラー")
                    lines.append(f'<p style="color:#b02020">❌ {esc(error)}</p>')
            lines.append("</div>")
        lines.append("</div>")
    else:
        lines.append("<p class='missing'>命令カウントデータなし</p>")

    lines.append("</div></div>")
    return "\n".join(lines)


# ===== 単体サマリテーブル =====

def generate_summary_table(label, data):
    """単体レポート用のサマリテーブル HTML を生成する。"""
    profiles = data.get("profiles", [])
    lines = []
    lines.append('<div class="table-wrap">')
    lines.append('<table class="sortable">')
    lines.append('<thead><tr>')
    lines.append('<th>Name</th><th>Result</th>')
    for col_name, _ in METRICS:
        lines.append(f'<th>{esc(col_name)}</th>')
    lines.append('</tr></thead>')
    lines.append('<tbody>')
    for i, entry in enumerate(profiles):
        name = entry.get("name", f"#{i}")
        ex = get_execution(entry)
        row_class = "" if ex or not entry.get("error") else ' class="error-row"'
        lines.append(f'<tr{row_class}>')
        lines.append(f'<td class="name-col" data-detail="detail-{i}">{esc(name)}</td>')
        if ex:
            result = ex.get("result", "?")
            lines.append(f'<td class="result-col">{esc(result)}</td>')
            for _, fn in METRICS:
                val = fn(ex)
                display = str(val) if val is not None else "<span class='missing'>N/A</span>"
                data_v = str(val) if val is not None else ""
                lines.append(f'<td data-v="{esc(data_v)}">{display}</td>')
        else:
            error = entry.get("error", "compile error")
            lines.append(f'<td colspan="{len(METRICS) + 1}" style="text-align:left">❌ {esc(error)}</td>')
        lines.append('</tr>')
    lines.append('</tbody></table></div>')
    return "\n".join(lines)


# ===== 比較テーブル =====

def generate_comparison_tables(inputs):
    """
    複数のプロファイル結果の比較テーブルを生成する。
    inputs: list[(label, data)]
    """
    if not inputs:
        return ""

    labels = [lbl for lbl, _ in inputs]
    # 全テストケース名の収集（順序保持）
    all_names = []
    seen = set()
    for _, data in inputs:
        for entry in data.get("profiles", []):
            n = entry.get("name", "")
            if n not in seen:
                all_names.append(n)
                seen.add(n)

    # name -> entry のマップを各ラベルごとに作成
    name_to_entry = {}
    for lbl, data in inputs:
        name_to_entry[lbl] = {e.get("name", ""): e for e in data.get("profiles", [])}

    lines = []
    # 各メトリクスごとのテーブル
    for col_name, metric_fn in METRICS:
        lines.append(f'<h2>{esc(col_name)}</h2>')
        lines.append('<div class="table-wrap">')
        lines.append('<table class="sortable">')
        lines.append('<thead><tr><th>Name</th>')
        for lbl in labels:
            lines.append(f'<th>{esc(lbl)}</th>')
        # 差分カラム（2ファイル以上のとき）
        if len(labels) >= 2:
            lines.append(f'<th>Diff ({esc(labels[-1])} vs {esc(labels[0])})</th>')
        lines.append('</tr></thead>')
        lines.append('<tbody>')
        for i, name in enumerate(all_names):
            row_entries = [name_to_entry[lbl].get(name) for lbl in labels]
            vals = [get_metric(e, metric_fn) if e else None for e in row_entries]
            lines.append(f'<tr>')
            lines.append(f'<td class="name-col" data-detail="detail-{i}">{esc(name)}</td>')
            for v in vals:
                display = str(v) if v is not None else "<span class='missing'>N/A</span>"
                data_v = str(v) if v is not None else ""
                lines.append(f'<td data-v="{esc(data_v)}">{display}</td>')
            if len(labels) >= 2:
                diff_info = compute_diff(vals[0], vals[-1])
                lines.append(f'<td>{format_diff(diff_info)}</td>')
            lines.append('</tr>')
        lines.append('</tbody></table></div>')
    return "\n".join(lines)


# ===== メイン HTML 生成 =====

def generate_html(inputs):
    """
    inputs: list[(label, data)]  data は load_profile() の返り値
    """
    is_comparison = len(inputs) > 1
    now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    parts = [HTML_HEADER]

    if is_comparison:
        parts.append("<h1>Whitespace VM Profile Comparison</h1>")
        sources = " vs ".join(esc(lbl) for lbl, _ in inputs)
        parts.append(f'<p class="subtitle">Sources: {sources} &nbsp;|&nbsp; Generated: {now}</p>')
    else:
        label = inputs[0][0]
        parts.append("<h1>Whitespace VM Profile Report</h1>")
        parts.append(f'<p class="subtitle">Source: {esc(label)} &nbsp;|&nbsp; Generated: {now}</p>')

    # サマリ / 比較テーブル
    if is_comparison:
        parts.append("<h2>Comparison Tables</h2>")
        parts.append(generate_comparison_tables(inputs))
    else:
        parts.append("<h2>Summary</h2>")
        parts.append(generate_summary_table(inputs[0][0], inputs[0][1]))

    # 詳細セクション
    parts.append("<h2>Details</h2>")
    if is_comparison:
        # 全テストケース名を収集
        all_names = []
        seen = set()
        for _, data in inputs:
            for entry in data.get("profiles", []):
                n = entry.get("name", "")
                if n not in seen:
                    all_names.append(n)
                    seen.add(n)
        name_to_entry = {}
        for lbl, data in inputs:
            name_to_entry[lbl] = {e.get("name", ""): e for e in data.get("profiles", [])}
        for i, name in enumerate(all_names):
            entries_by_label = {lbl: name_to_entry[lbl].get(name) for lbl in [l for l, _ in inputs]}
            parts.append(generate_comparison_detail(name, entries_by_label, i))
    else:
        data = inputs[0][1]
        for i, entry in enumerate(data.get("profiles", [])):
            parts.append(generate_single_detail(entry, i))

    parts.append(HTML_FOOTER)
    return "\n".join(parts)


# ===== エントリポイント =====

def main():
    args = sys.argv[1:]
    if not args:
        print_usage()
        sys.exit(1)

    inputs_spec, output_path = parse_args(args)

    if not inputs_spec:
        print("Error: 入力ファイルを1つ以上指定してください", file=sys.stderr)
        sys.exit(1)

    # JSON 読み込み
    inputs = []
    for label, path in inputs_spec:
        try:
            data = load_profile(path)
        except FileNotFoundError:
            print(f"Error: ファイルが見つかりません: {path}", file=sys.stderr)
            sys.exit(1)
        except json.JSONDecodeError as e:
            print(f"Error: JSON パースエラー ({path}): {e}", file=sys.stderr)
            sys.exit(1)
        inputs.append((label, data))

    html_content = generate_html(inputs)

    # 出力
    if output_path:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(html_content)
        print(f"レポートを書き出しました: {output_path}", file=sys.stderr)
    else:
        sys.stdout.write(html_content)


if __name__ == "__main__":
    main()
