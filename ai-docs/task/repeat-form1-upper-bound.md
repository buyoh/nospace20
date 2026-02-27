# repeat Form 1 の意味論変更: ループ回数 → 上限値

## 概要

`repeat: i(init), N, body;` の意味を「N 回ループ」から「`i < N` の間ループ」に変更する。

## 動機

- 現在の実装では隠し変数 `__rpt_n` を用いたカウントダウン方式で、body 内で `i` を変更してもループ回数が変化しない
- 一般的なプログラミング言語の for ループに合わせ、`i < N` の間ループとするのが自然
- `repeat: i(1), 5, __clog(i);` → `1, 2, 3, 4` と表示される（`i < 5` の間）

## 変更前後の比較

### 脱糖の変更

**変更前:**
```
repeat: i(init), N, body;
→ for: { let: i(init); let: __rpt_n(N); } { __rpt_n > 0; } { i += 1; __rpt_n -= 1; } { body; };
```

**変更後:**
```
repeat: i(init), N, body;
→ for: { let: i(init); } { i < N; } { i += 1; } { body; };
```

### 意味論の変化

| ケース | 変更前（ループ回数） | 変更後（上限値） |
|--------|---------------------|-----------------|
| `repeat: i(0), 5, ...` | 5回 (i=0,1,2,3,4) | 5回 (i=0,1,2,3,4) |
| `repeat: i(1), 5, ...` | 5回 (i=1,2,3,4,5) | 4回 (i=1,2,3,4) |
| `repeat: i(0), 0, ...` | 0回 | 0回 |
| `repeat: i(3), 3, ...` | 3回 (i=3,4,5) | 0回 (3 < 3 は偽) |
| body 内で `i` を変更 | ループ回数に影響しない | ループ回数に影響する |

### N 式の再評価

N は条件ブロックで毎回評価される（`{ i < N; }`）。N が変数の場合、外部から変更されればループ回数が変化する。これは C 言語の `for(i=init; i<N; i++)` と同等の動作。

## 実装変更

### tree_parser（`src/tree_parser/statement/mod.rs`）

1. **`desugar_repeat_form1` 関数の変更:**
   - `rpt_n_name` パラメータを削除
   - init ブロック: `__rpt_n` 変数宣言を削除、`i` のみ
   - cond ブロック: `__rpt_n > 0` → `i < N`（`Operator2::Less` を使用）
   - step ブロック: `__rpt_n -= 1` を削除、`i += 1` のみ

2. **`parse_to_statements_repeat` メソッドの変更:**
   - Form 1 で `rpt_n_name`/`repeat_counter` の使用を削除

3. **`repeat_counter` フィールドの削除:**
   - `StatementBuilder` から `repeat_counter: usize` を削除
   - もはやどの Form でも使用されない

### テスト更新

既存テストは `repeat: i(0), N, ...` 形式のため、`i < N` でも結果は同じ（0 始まりではループ回数 = 上限値）。ただし意味論の正確性のため:

- `repeat_form1_001.ns` のコメントを更新
- `repeat_nested_001.ns` は変更不要（`repeat: i(0), 3, ...` は同じ結果）

### 影響範囲

- semantic_analyzer: 変更不要（Statement::For の構造は同じ）
- interpreter: 変更不要
- compiler_ws: 変更不要
- optimizer: 変更不要

tree_parser の脱糖ヘルパーのみの変更で完結する。
