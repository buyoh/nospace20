# Whitespace プロファイラ HTML レポート

## 概要

`ws_profiler` (examples/ws_profiler.rs) が出力するプロファイル結果を HTML レポートにまとめるスクリプトを作成する。
過去の結果と並べて比較できる機能も備える。

## 背景

### 現状

- `ws_profiler` は YAML 形式でプロファイル結果を標準出力する
- 結果を人間が読みやすく俯瞰する手段がない
- 過去の結果との差分比較（リグレッション検出等）ができない

### 目標

1. プロファイル結果から HTML サマリレポートを生成するスクリプトを作成
2. 複数のプロファイル結果を並べて比較できるようにする
3. 外部依存を最小化する（Python 標準ライブラリのみ）

## 設計

詳細は以下のドキュメントを参照:

| ドキュメント | 内容 |
|---|---|
| [profiler-json-output.md](profiler-json-output.md) | ws_profiler への JSON 出力オプション追加 |
| [html-report-script.md](html-report-script.md) | HTML レポート生成スクリプトの設計 |

## 実装計画

### Phase 1: ws_profiler に JSON 出力を追加

- `--json` フラグで JSON 形式の出力をサポート
- Python スクリプトが外部依存なし（stdlib の `json` モジュール）で読み込めるようにする
- 既存の YAML 出力はデフォルトのまま維持

### Phase 2: HTML レポート生成スクリプト

- `tools/profile-report.py` を作成
- JSON ファイルを1つ以上受け取り、比較可能な HTML レポートを生成
- スタンドアロン HTML（外部 CSS/JS 依存なし）
- 差分表示・色分け・ソート機能

## 設計原則

1. **外部依存ゼロ**: Python 標準ライブラリのみ使用（PyYAML 不要）
2. **スタンドアロン HTML**: 生成される HTML は単一ファイルで完結（CSS/JS 埋め込み）
3. **既存互換**: `ws_profiler` のデフォルト動作（YAML 出力）は変更しない
4. **比較が主機能**: 1ファイルでの要約と、複数ファイルの並列比較の両方をサポート
