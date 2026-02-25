# Whitespace プロファイラ

## 概要

whitespace20 VM に実行プロファイラ機能を追加し、実行統計（ステップ数・メモリアクセス範囲等）を収集・YAML 出力するスクリプトを作成する。

## 背景

### 現状

- `WhitespaceVM` は `total_steps` のみをメトリクスとして持つ
- `whitespace20` CLI は `--debug` で基本的な実行情報（total_steps, stack size, heap size）を表示するのみ
- nospace テストケース（`resources/tests/passes/`）を Whitespace にコンパイルした際の実行特性を定量的に把握する手段がない

### 目標

1. VM レベルでプロファイリングデータを収集する仕組みを導入
2. テストケースをピックアップして統計を YAML で出力する Rust スクリプト（examples バイナリ）を作成

## 設計

詳細は以下のドキュメントを参照:

| ドキュメント | 内容 |
|---|---|
| [profiler-design.md](profiler-design.md) | プロファイラのデータ構造と VM への統合設計 |
| [profiler-script.md](profiler-script.md) | YAML 出力スクリプトの設計 |

## 実装計画

### Phase 1: ProfileStats 構造体の追加と VM 統合

- `ProfileStats` 構造体を `src/whitespace/profiler.rs` に追加
- `WhitespaceVM` にプロファイリングモードを追加（`with_profiling(bool)` ビルダー）
- `execute_instruction` 内でプロファイリングデータを収集
- Unit テスト

### Phase 2: YAML 出力スクリプト

- `examples/ws_profiler.rs` に Rust スクリプトを作成
- テストケースの自動ピックアップ（`resources/tests/passes/` から）
- nospace → Whitespace コンパイル → VM 実行 → 統計収集 → YAML 出力
- serde_yaml を dev-dependencies に追加

## 設計原則

1. **プロファイリングはオプトイン**: `with_profiling(true)` 時のみデータ収集。無効時にオーバーヘッドを最小化
2. **既存動作に影響なし**: プロファイリングの有無で実行結果は変わらない
3. **VM 内部完結**: プロファイリングデータは VM の構造体に蓄積。外部フックは不要
