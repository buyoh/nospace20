# テスト check.json の `trace` 属性名・型の改善

## 背景・課題

`resources/tests/passes/*.check.json` で使われている `"trace"` 属性には以下の問題がある:

1. **名前が曖昧**: `"trace"` はイベントログや実行シーケンスを連想させるが、実際は「各トレースポイントのヒット回数」の配列
2. **型の意味が暗黙的**: `[2, 1, 1, 1]` は配列だが、インデックスが `__trace(n)` の引数に対応し、値がその呼び出し回数であることが暗黙的
3. **ドキュメントを読まないと理解不能**: 初見では `"trace": [2, 1, 1, 1]` が何を意味するか推測困難

### 現状の形式

```json
{ "trace": [2, 1, 1, 1] }
```

意味: `__trace(0)` が 2 回、`__trace(1)` が 1 回、`__trace(2)` が 1 回、`__trace(3)` が 1 回実行された。

## 提案

### 属性名の変更: `trace` → `trace_hit_counts`

```json
{ "trace_hit_counts": [2, 1, 1, 1] }
```

**理由:**
- `hit_counts` により、値が「回数」であることが明確
- `trace_` プレフィックスにより、`__trace()` 関数との関連が明確
- 読み手は「各トレースポイントがヒットした回数」と即座に理解可能

### 型（配列構造）は維持

配列構造 `Vec<i64>` は変更しない。理由:

- 大半のテストケースでトレースポイントは 0 から連番で使用されており、配列が自然
- マップ形式 `{"0": 2, "1": 1}` は冗長でファイル数が多い現状ではノイズが増える
- 属性名を改善すれば、配列の各要素が「インデックス i = `__trace(i)` のヒット回数」であることは十分推測可能

### 後方互換性

既存の `"trace"` フィールドも引き続きサポートする。`serde(alias)` を利用:

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum TestConfig {
    Success {
        #[serde(alias = "trace")]
        trace_hit_counts: Vec<i64>,
    },
    // ...
}
```

レガシーパーサー (`from_legacy`) も `"trace"` を `trace_hit_counts` にマッピングするよう更新。

## 変更対象

### 1. Rust コード

| ファイル | 変更内容 |
|---------|---------|
| `tests/code_test.rs` | `TestConfig::Success { trace }` → `TestConfig::Success { trace_hit_counts }`, `serde(alias = "trace")` 追加, 検証ロジックの変数名更新 |

### 2. check.json ファイル (約 80+ ファイル)

`"trace":` → `"trace_hit_counts":` にリネーム。一括 sed で対応可能:

```bash
find resources/tests -name '*.check.json' -exec sed -i '' 's/"trace":/"trace_hit_counts":/g' {} +
```

`"type": "success"` を明示しているファイルとそうでないファイル（レガシー形式）の両方が対象。

### 3. ドキュメント

| ファイル | 変更内容 |
|---------|---------|
| `resources/tests/README.md` | 属性名・説明の更新 |
| `.github/skills/add-test-spec/SKILL.md` | `trace` の参照がある場合は更新 |

## 作業手順

1. `tests/code_test.rs` の `TestConfig::Success` フィールド名を `trace_hit_counts` に変更し、`serde(alias = "trace")` を追加
2. 検証ロジック内の変数名を更新
3. 全 `.check.json` ファイルの `"trace":` を `"trace_hit_counts":` に一括置換
4. `resources/tests/README.md` を更新
5. `cargo test` で全テスト通過を確認

## ステータス

- [x] 設計完了
- [x] 実装
- [x] テスト確認（127 passed, 21 ignored）
- [x] ドキュメント更新

## 実施内容

### 実装完了 (2026-02-16)

1. **Rust コード更新** (`tests/code_test.rs`)
   - `TestConfig::Success` フィールド名を `trace` → `trace_hit_counts` に変更
   - `#[serde(alias = "trace")]` を追加して後方互換性を確保
   - `from_legacy` メソッドを更新し、`trace` と `trace_hit_counts` の両方に対応
   - 検証ロジック内の変数名を更新

2. **check.json 一括変換**
   - `tools/rename-trace-to-trace-hit-counts.sh` スクリプトを作成
   - 約83ファイルの `"trace":` を `"trace_hit_counts":` に一括変換
   - macOS / Linux 両対応

3. **ドキュメント更新**
   - `resources/tests/README.md` の `trace` → `trace_hit_counts` に更新
   - `.github/skills/add-test-spec/SKILL.md` は `trace` の参照なし（更新不要）

4. **テスト実行**
   - `cargo test --test code_test` で全テスト通過確認
   - 127 passed, 21 ignored (requires wsc)

5. **コミット**
   - コミットハッシュ: 808068a
   - 87 files changed, 123 insertions(+), 95 deletions(-)

### 後方互換性

以下の方法で後方互換性を確保:

1. `#[serde(alias = "trace")]` により、旧形式 `"trace"` も引き続き読み込み可能
2. `from_legacy` メソッドが `"trace"` と `"trace_hit_counts"` の両方を検出

## 完了
