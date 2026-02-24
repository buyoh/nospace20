# メモリアロケータ実装

## 概要

Whitespace コンパイラにランタイムメモリアロケータを導入する。現在のスタックフレーム管理（`LOCAL_HEAP_BEGIN`/`LOCAL_HEAP_END` によるバンプ方式）をアロケータベースに移行し、最終的にはユーザーコードからの動的ヒープ確保（`__alloc`/`__free`）を可能にする。

この機能は Whitespace コンパイル時にとって非常に複雑であるため、`--std-ext alloc` による明示的な有効化時のみ利用可能とする。

## 背景

### 現状の課題

1. **動的メモリ確保が不可能**: 配列サイズはコンパイル時定数のみ。実行時にサイズが決まるバッファを確保する手段がない
2. **セルフコンパイラの制約**: AST ノード等を固定長配列＋手動インデックスで管理するしかない
3. **ローカルフレームが一方通行**: `LOCAL_HEAP_END` は関数呼び出しで伸長するのみで、フレーム間の隙間を再利用しない
4. **メモリ管理の抽象化不足**: スタックフレームとヒープ領域が別々の仕組みで管理されている

### 目標

| 段階 | 内容 |
|------|------|
| 1. アロケータ基盤 | Whitespace サブルーチンとしてメモリアロケータを実装 |
| 2. スタックフレーム移行 | 関数呼び出し時のフレーム確保をアロケータ経由に変更 |
| 3. ヒープ API 公開 | `__alloc(size)` / `__free(ptr)` を nospace 組み込み関数として提供 |

## ドキュメント

| ドキュメント | 内容 |
|---|---|
| [allocator-design.md](allocator-design.md) | コアアロケータのアルゴリズムとデータ構造 |
| [fixed-size-block-allocator.md](fixed-size-block-allocator.md) | 固定サイズブロックアロケータ (FSBA) の詳細設計 |
| [heap-layout.md](heap-layout.md) | 新しいヒープメモリレイアウト設計 |
| [compiler-changes.md](compiler-changes.md) | compiler_ws モジュールへの変更設計 |
| [std-ext-integration.md](std-ext-integration.md) | `--std-ext alloc` の統合と条件分岐 |
| [testing-strategy.md](testing-strategy.md) | テスト計画 |

## Phase 一覧

| Phase | 内容 | 依存 | 規模 |
|---|---|---|---|
| Phase 1 | `--std-ext alloc` の追加と基盤整備 | なし | 小 |
| Phase 2 | アロケータサブルーチンのコード生成 | Phase 1 | 大 |
| Phase 3 | スタックフレーム確保をアロケータ経由に変更 | Phase 2 | 中 |
| Phase 4 | `__alloc`/`__free` 組み込み関数の公開 | Phase 2 | 中 |
| Phase 5 | テスト・検証 | Phase 3, 4 | 中 |

## 設計原則

1. **`--std-ext alloc` 未指定時は既存動作を維持**: 後方互換性を保証
2. **Whitespace 命令セットのみで実装**: アロケータ自体が Whitespace サブルーチンとして動作
3. **段階的な移行**: まずアロケータ基盤を作り、次にスタックフレーム、最後にユーザー API
4. **インタプリタは影響なし**: メモリアロケータは Whitespace コンパイル時のみの機能

## 現状

- 2026-02-24: 設計ドキュメント作成
- 2026-02-24: 固定サイズブロックアロケータ (FSBA) 設計追加。二層アーキテクチャ (FSBA + 汎用 First-Fit) に変更
