# Phase 4: wsc テストの `--unchecked-heap` 除去を検討

## 概要

現在、外部インタプリタ `wsc` を使ったテスト（`whitespace` ターゲット）では `--unchecked-heap` フラグを渡して未初期化ヒープエラーを回避している。strict-heap テストが組み込み VM で安定的に通ることを確認した後、`wsc` テストからも `--unchecked-heap` を除去し、wsc のデフォルト動作（strict-heap）でテストを実行する。

## 対象ファイル

| ファイル | 変更内容 | 規模 |
|---------|---------|------|
| `tests/common/mod.rs` | `run_whitespace` から `--unchecked-heap` を除去 | 小 |

## 現在のコード

```rust
// tests/common/mod.rs:78-80
let mut child = Command::new(&wsc_path)
    .arg(file.path())
    .args(["--unchecked-heap"]) // heap は0クリアで開始とみなす
    // ...
```

## 変更方針

### 前提条件

Phase 3 の strict-heap テスト（`whitespace-self-strict` ターゲット）で全テストが通ることが確認されている必要がある。strict-heap で失敗するテストが `exclude_targets: [whitespace-self-strict]` で除外されている場合、同じテストは wsc でも失敗する可能性がある。

### 選択肢

#### A: `--unchecked-heap` を完全に除去

```rust
let mut child = Command::new(&wsc_path)
    .arg(file.path())
    // --unchecked-heap を削除
    .stdin(Stdio::piped())
    // ...
```

- **利点**: wsc のデフォルト動作でテストでき、strict-heap モードの正当性を外部インタプリタで確認できる
- **リスク**: wsc と組み込み VM の挙動差異で追加の失敗が発生する可能性

#### B: `--unchecked-heap` を残し、strict-heap テストは別途 wsc 版も用意

wsc テスト用に strict/non-strict の両バリアントを生成する。

- **利点**: 既存の wsc テストが壊れない
- **欠点**: 複雑性が増す

### 推奨: 段階的アプローチ

1. まず Phase 3 で `whitespace-self-strict` テストを安定させる
2. `whitespace-self-strict` で除外されたテストのリストを確認
3. 除外リストが空または少数なら、選択肢 A（`--unchecked-heap` 完全除去）を実施
4. 除外リストが多い場合は、コンパイラ側の初期化コード改善を先に行う

## 更新履歴

- 2026-02-18: 初版作成
