# Phase 5: 変数の初期値を「未定義」に変更する

## 背景

現在の仕様・実装:
- spec.md (セクション4): 「変数は初期値 0 で初期化される。（TODO: 未定義に変更）。」
- nospace インタプリタ: `vec![0; variable_count]` でゼロ初期化
- Whitespace コンパイラ: `generate_local_allocate` はヒープポインタを進めるだけでゼロクリアしない（元からヒープに残っている値が見える）

つまり、**インタプリタは 0 で初期化しているが、Whitespace コンパイルでは実質未定義**という不一致がある。仕様を「未定義」に統一し、テストで検出可能にする。

## 仕様変更

### spec.md の変更箇所

```diff
- 変数は初期値 0 で初期化される。（TODO: 未定義に変更）。
+ 変数の初期値は未定義である。初期値を指定しない場合、読み出し時の値は不定となる。
```

```diff
- let: z;              # z を 0 で初期化 #
+ let: z;              # z の初期値は未定義 #
```

```diff
- __clog(a);  # 初期値 0 で初期化（TODO: 未定義に変更） #
+ __clog(a);  # 初期値は未定義 #
```

## 実装方法

変数の初期値を「未定義」にするには、**インタプリタと Whitespace VM の両方で、未初期化変数の読み出しを検出できる仕組みが必要**。

### 方法: 初期化フラグ方式（推奨）

#### nospace インタプリタ側

変数ストレージを `Vec<i64>` から `Vec<Option<i64>>` に変更し、未初期化のまま読み出した場合の動作を制御可能にする。

```rust
// 現在の実装
let mut variables = vec![0; func.block.scope.variable_count];

// 変更後
let mut variables = vec![None; func.block.scope.variable_count];
```

ただし、パフォーマンスへの影響と変更範囲が大きいため、**段階的に導入**する。

**ステップ A: `Option<i64>` 化は行わず、初期値のみ変更**

まず `vec![0; count]` の初期値をモードによって切り替える:

```rust
// 通常モード: 0 で初期化（後方互換）
let mut variables = vec![0i64; func.block.scope.variable_count];

// strict モード: ランダム値で初期化（Phase 6 で詳述）
let mut variables: Vec<i64> = (0..func.block.scope.variable_count)
    .map(|_| random_fill_value())
    .collect();
```

**ステップ B: 将来的に `Option<i64>` へ移行（任意）**

`Option<i64>` にすると未初期化読み出しを明確にエラーにできるが、インタプリタのコード全体に影響するため、別タスクとする。

#### Whitespace VM / コンパイラ側

Whitespace コンパイラは `generate_local_allocate` でヒープポインタを進めるだけでゼロクリアしていない。この挙動は「未定義」仕様と整合している。

strict-heap モード（Phase 1）で未初期化ヒープアクセスがエラーとなるため、変数初期化忘れのバグは Phase 1-3 の仕組みで検出可能。

### 対象ファイル

| ファイル | 変更内容 | 規模 |
|---------|---------|------|
| `spec.md` | 初期値の記述を「未定義」に変更 | 小 |
| `src/interpreter/exec.rs` | `new_func`, `enter_block` の初期値 | 中 |
| `src/interpreter/mod.rs` | `interpret_global`, `initialize_function_statics` の初期値 | 中 |

## テストへの影響

- 初期値 0 を暗黙的に前提としている既存テストがある場合、失敗する
- Phase 6 のランダム初期化モードで検出可能

## 依存関係

- Phase 6（ランダム初期化モード）と組み合わせて導入する
- Phase 1-3（strict-heap）が先に完了していることが望ましい

## 更新履歴

- 2026-02-18: 初版作成
