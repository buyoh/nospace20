# Phase 4 実装レポート: Whitespace コンパイラの配列対応

**実装日**: 2026-02-13
**状態**: 完了

## 実装内容

### 1. メモリレイアウトの拡張 (`src/compiler_ws/memory.rs`)

配列のために複数スロットを確保できるよう、`allocate_global_slots` メソッドを追加しました。

```rust
pub fn allocate_global_slots(&mut self, size: i64) -> HeapAddress {
    let addr = Self::GLOBAL_PTR.offset(self.global_var_count);
    self.global_var_count += size;
    addr
}
```

既存の `allocate_global` は後方互換性のために残し、内部で `allocate_global_slots(1)` を呼ぶようにしました。

### 2. 配列アクセスのコード生成 (`src/compiler_ws/expression.rs`)

#### 2.1 配列要素の読み取り

`generate_array_access` 関数を実装し、`arr[index]` の値をスタックにプッシュする処理を追加：

- グローバル配列: `base_addr = GLOBAL_PTR + offset + index` で計算
- ローカル配列: `heap[LOCAL_HEAP_BEGIN] + offset + index` で計算

#### 2.2 配列要素への代入

`generate_store_array` 関数を実装し、`arr[index] = value` の処理を追加：

- アドレスを計算してスタックに積む
- 値を評価してストア
- 代入式の値として、ストアした値を再度取得してスタックに残す

#### 2.3 代入演算子の拡張

`Operator2::Assign` のパターンマッチを拡張し、左辺が `ArrayAccess` の場合も処理できるようにしました。

### 3. テストマニフェストの更新

`resources/tests/test-manifest.yaml` において、以下のテストに `whitespace` ターゲットを追加：

- `test_array_basic`: 配列の宣言・アクセス・初期化
- `test_array_static`: static 配列の操作

**注意**: `test_array_reference` は参照演算子 (`&`) を使用しているため、Whitespace ターゲットには追加しませんでした。参照演算子は Phase 4 では未実装です。

## テスト結果

### コンパイルテスト

以下のテストケースのコンパイルが成功することを確認：

- `array-basic.ns`: ✓ コンパイル成功
- `array-static.ns`: ✓ コンパイル成功
- `array-reference.ns`: ✗ 参照演算子未実装のためスキップ

### 全体テスト

```
cargo test
```

結果: **109 passed; 0 failed; 16 ignored**

既存のすべてのテストが引き続き通過しています。

## 未実装項目

### 参照演算子 (`&`) と参照外し演算子 (`*`)

`expression.rs` の `generate_unary_op` 内で `Operator1::Ref` と `Operator1::Deref` は `unimplemented!()` のままです。

これらの演算子は Phase 4 の範囲外であり、配列へのポインタ操作（`&arr[i]` など）を含むテストは Whitespace コンパイラでは実行できません。

### 境界チェック

Phase 4 の設計方針に従い、Whitespace コンパイラでは配列の境界チェックを省略しています。これは：

- Whitespace の命令セットでは境界チェックのコード量が大きくなる
- インタプリタで十分にテストされたコードをコンパイルする想定

という理由によります。

## 次のステップ

- Phase 5: 文字列リテラル（糖衣構文）の実装
- または、参照演算子の実装（別フェーズとして）

## 変更ファイル

- `src/compiler_ws/memory.rs`
- `src/compiler_ws/expression.rs`
- `resources/tests/test-manifest.yaml`

## まとめ

Phase 4 として計画されていた Whitespace コンパイラの配列対応が完了しました。配列の宣言、アクセス、代入がコンパイルできるようになり、グローバル配列とローカル配列の両方をサポートしています。

参照演算子は未実装のため、`array-reference` テストは Whitespace ターゲットから除外しましたが、基本的な配列操作は正常に機能しています。
