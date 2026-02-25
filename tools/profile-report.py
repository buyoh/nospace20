#!/usr/bin/env python3
"""
Whitespace VM Profile HTML Report Generator

Accepts one or more JSON files output by `ws_profiler --json` and generates
a comparable HTML report.

Usage:
    # Single report
    python3 tools/profile-report.py tmp/profile.json -o tmp/report.html

    # Compare two results
    python3 tools/profile-report.py tmp/before.json tmp/after.json -o tmp/compare.html

    # Labeled comparison
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


# ===== Argument parsing =====

def print_usage():
    print("""usage: profile-report.py [-h] [-o OUTPUT] [--label LABEL] input [input ...]

positional arguments:
  input          Profile JSON file(s) (one or more)

optional arguments:
  -h, --help     Show this help message
  -o OUTPUT      Output HTML file path (default: stdout)
  --label LABEL  Label for the immediately following input file (default: filename)
""", file=sys.stderr)


def parse_args(argv):
    """
    Parse arguments and return (inputs: list[(label, path)], output: str|None).
    --label LABEL binds to the immediately following positional argument.
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


# ===== JSON loading =====

def load_profile(path):
    """Load a JSON file and return a dict. Expected format: {"profiles": [...]}"""
    with open(path, encoding='utf-8') as f:
        return json.load(f)


# ===== Metrics extraction =====

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
    """Return the execution dict from a profile entry, or None if absent."""
    return profile_entry.get("execution")


def get_metric(profile_entry, metric_fn):
    """Extract a metric value from a profile entry. Returns None if unavailable."""
    ex = get_execution(profile_entry)
    if ex is None:
        return None
    return metric_fn(ex)


# ===== Diff computation =====

def compute_diff(before, after):
    """Compute the numeric difference and percentage change between two values."""
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
    """Format a diff dict as a colored HTML string."""
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
    # Higher values are worse (red); lower values are better (green)
    css_class = "worse" if d > 0 else "better"
    arrow = "▲" if d > 0 else "▼"
    return f'<span class="{css_class}">{sign}{d} ({pct_str}) {arrow}</span>'


def esc(s):
    """HTML-escape a string."""
    return html.escape(str(s))


# ===== HTML constants =====

HTML_HEADER = """<!DOCTYPE html>
<html lang="en">
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
  tfoot tr { background: #e8edf5 !important; font-weight: bold; }
  tfoot td { border-top: 2px solid #aaa; }
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
// Table sort
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

// Detail section expand/collapse
document.querySelectorAll('.detail-header').forEach(function(header) {
  header.addEventListener('click', function() {
    var body = header.nextElementSibling;
    body.classList.toggle('open');
  });
});

// Click on name column to expand detail section
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


# ===== Instruction bar chart =====

def generate_instr_bars(instr_counts, max_val=None):
    """Generate HTML bar chart for per-instruction execution counts."""
    if not instr_counts:
        return "<p class='missing'>No instruction counts</p>"
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


# ===== Stat display =====

def stat_item(label, value):
    if value is None:
        value_html = "<span class='missing'>N/A</span>"
    else:
        value_html = f"<span class='stat-value'>{esc(value)}</span>"
    return f"<div class='stat-item'><div class='stat-label'>{esc(label)}</div>{value_html}</div>"


# ===== Single detail section =====

def generate_single_detail(entry, idx):
    """Generate detail section HTML for a single-file report."""
    name = entry.get("name", f"entry-{idx}")
    detail_id = f"detail-{idx}"
    ex = get_execution(entry)

    lines = []
    lines.append(f'<div class="detail-section">')
    lines.append(f'<div class="detail-header">▶ {esc(name)}</div>')
    lines.append(f'<div class="detail-body" id="{detail_id}">')

    if ex is None:
        error = entry.get("error", "Unknown error")
        lines.append(f'<p style="color:#b02020">&#x274C; {esc(error)}</p>')
    else:
        result = ex.get("result", "?")
        lines.append(f'<p><strong>Result:</strong> {esc(result)} &nbsp; <strong>Total Steps:</strong> {ex.get("total_steps", "N/A")}</p>')

        # Stats grid
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

        # Instruction bar chart
        lines.append("<h3>Instruction execution counts</h3>")
        ic = ex.get("instruction_counts", {})
        lines.append(generate_instr_bars(ic))

    lines.append("</div></div>")
    return "\n".join(lines)


# ===== Comparison detail section =====

def generate_comparison_detail(name, entries_by_label, idx):
    """Generate detail section HTML for a comparison report."""
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
                lines.append("<p class='missing'>No data</p>")
            else:
                ex = get_execution(entry)
                if ex:
                    ic = ex.get("instruction_counts", {})
                    lines.append(generate_instr_bars(ic))
                else:
                    error = entry.get("error", "Unknown error")
                    lines.append(f'<p style="color:#b02020">&#x274C; {esc(error)}</p>')
            lines.append("</div>")
        lines.append("</div>")
    else:
        lines.append("<p class='missing'>No instruction count data</p>")

    lines.append("</div></div>")
    return "\n".join(lines)


# ===== Single summary table =====

def generate_summary_table(label, data):
    """Generate summary table HTML for a single-file report, including a totals row."""
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
    # Accumulate column totals (None if any value is missing)
    col_totals = [0] * len(METRICS)
    col_valid = [True] * len(METRICS)
    for i, entry in enumerate(profiles):
        name = entry.get("name", f"#{i}")
        ex = get_execution(entry)
        row_class = "" if ex or not entry.get("error") else ' class="error-row"'
        lines.append(f'<tr{row_class}>')
        lines.append(f'<td class="name-col" data-detail="detail-{i}">{esc(name)}</td>')
        if ex:
            result = ex.get("result", "?")
            lines.append(f'<td class="result-col">{esc(result)}</td>')
            for j, (_, fn) in enumerate(METRICS):
                val = fn(ex)
                display = str(val) if val is not None else "<span class='missing'>N/A</span>"
                data_v = str(val) if val is not None else ""
                lines.append(f'<td data-v="{esc(data_v)}">{display}</td>')
                if val is not None:
                    col_totals[j] += val
                else:
                    col_valid[j] = False
        else:
            error = entry.get("error", "compile error")
            lines.append(f'<td colspan="{len(METRICS) + 1}" style="text-align:left">&#x274C; {esc(error)}</td>')
            for j in range(len(METRICS)):
                col_valid[j] = False
        lines.append('</tr>')
    lines.append('</tbody>')
    # Totals footer row
    lines.append('<tfoot><tr>')
    lines.append('<td class="name-col" style="cursor:default"><strong>Total</strong></td>')
    lines.append('<td></td>')  # Result column
    for j in range(len(METRICS)):
        if col_valid[j]:
            lines.append(f'<td><strong>{col_totals[j]}</strong></td>')
        else:
            lines.append("<td><span class='missing'>—</span></td>")
    lines.append('</tr></tfoot>')
    lines.append('</table></div>')
    return "\n".join(lines)


# ===== Comparison tables =====

def generate_comparison_tables(inputs):
    """
    Generate per-metric comparison tables for multiple profile results.
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
    # One table per metric
    for col_name, metric_fn in METRICS:
        lines.append(f'<h2>{esc(col_name)}</h2>')
        lines.append('<div class="table-wrap">')
        lines.append('<table class="sortable">')
        lines.append('<thead><tr><th>Name</th>')
        for lbl in labels:
            lines.append(f'<th>{esc(lbl)}</th>')
        # Diff column when 2+ files are provided
        if len(labels) >= 2:
            lines.append(f'<th>Diff ({esc(labels[-1])} vs {esc(labels[0])})</th>')
        lines.append('</tr></thead>')
        lines.append('<tbody>')
        # Accumulators for totals row
        col_totals = [0] * len(labels)
        col_valid = [True] * len(labels)
        for i, name in enumerate(all_names):
            row_entries = [name_to_entry[lbl].get(name) for lbl in labels]
            vals = [get_metric(e, metric_fn) if e else None for e in row_entries]
            lines.append(f'<tr>')
            lines.append(f'<td class="name-col" data-detail="detail-{i}">{esc(name)}</td>')
            for k, v in enumerate(vals):
                display = str(v) if v is not None else "<span class='missing'>N/A</span>"
                data_v = str(v) if v is not None else ""
                lines.append(f'<td data-v="{esc(data_v)}">{display}</td>')
                if v is not None:
                    col_totals[k] += v
                else:
                    col_valid[k] = False
            if len(labels) >= 2:
                diff_info = compute_diff(vals[0], vals[-1])
                lines.append(f'<td>{format_diff(diff_info)}</td>')
            lines.append('</tr>')
        lines.append('</tbody>')
        # Totals footer row
        total_vals = [col_totals[k] if col_valid[k] else None for k in range(len(labels))]
        lines.append('<tfoot><tr>')
        lines.append('<td class="name-col" style="cursor:default"><strong>Total</strong></td>')
        for k, tv in enumerate(total_vals):
            if tv is not None:
                lines.append(f'<td><strong>{tv}</strong></td>')
            else:
                lines.append("<td><span class='missing'>—</span></td>")
        if len(labels) >= 2:
            total_diff = compute_diff(total_vals[0], total_vals[-1])
            lines.append(f'<td>{format_diff(total_diff)}</td>')
        lines.append('</tr></tfoot>')
        lines.append('</table></div>')
    return "\n".join(lines)


# ===== Main HTML generation =====

def generate_html(inputs):
    """
    Generate the full HTML report.
    inputs: list[(label, data)]  where data is the return value of load_profile()
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

    # Summary / comparison tables
    if is_comparison:
        parts.append("<h2>Comparison Tables</h2>")
        parts.append(generate_comparison_tables(inputs))
    else:
        parts.append("<h2>Summary</h2>")
        parts.append(generate_summary_table(inputs[0][0], inputs[0][1]))

    # Detail sections
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


# ===== Entry point =====

def main():
    args = sys.argv[1:]
    if not args:
        print_usage()
        sys.exit(1)

    inputs_spec, output_path = parse_args(args)

    if not inputs_spec:
        print("Error: Please specify at least one input file.", file=sys.stderr)
        sys.exit(1)

    # Load JSON files
    inputs = []
    for label, path in inputs_spec:
        try:
            data = load_profile(path)
        except FileNotFoundError:
            print(f"Error: File not found: {path}", file=sys.stderr)
            sys.exit(1)
        except json.JSONDecodeError as e:
            print(f"Error: JSON parse error ({path}): {e}", file=sys.stderr)
            sys.exit(1)
        inputs.append((label, data))

    html_content = generate_html(inputs)

    # Write output
    if output_path:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(html_content)
        print(f"Report written to: {output_path}", file=sys.stderr)
    else:
        sys.stdout.write(html_content)


if __name__ == "__main__":
    main()
