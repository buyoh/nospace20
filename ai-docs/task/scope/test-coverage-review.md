# スコープ機能のテストカバレッジレビュー

## 実施日

2026-02-04

## 既存のテストケース

### 有効化されているテスト

| テスト名 | ファイル | 内容 | カバレッジ |
|---------|---------|------|-----------|
| `scope_block_001` | `scope_block_001.ns` | ブロック内でのシャドウイング | ✅ シャドウイング基本 |
| `scope_func_001` | `scope_func_001.ns` | 関数スコープの独立性 | ✅ 関数スコープ分離 |

### 無効化されているテスト

| テスト名 | ファイル | 内容 | 理由 |
|---------|---------|------|------|
| `disabled_scope_block_var_001` | `disabled_scope_block_var_001.ns` | ブロック内変数定義 | Phase 1で実装済み（有効化可能） |
| `scope_nested_func_001` | `scope_nested_func_001.ns` | ネスト関数スコープ | 未実装機能（Phase 5） |

---

## Phase 1 実装済み機能のカバレッジ分析

### ✅ カバーされている機能

| 機能 | テストケース |
|------|-------------|
| シャドウイング（基本） | `scope_block_001` |
| 関数スコープの独立性 | `scope_func_001` |
| ブロック内変数定義 | `disabled_scope_block_var_001` |

### ❌ カバーされていない機能

| 機能 | 必要性 | 優先度 |
|------|--------|--------|
| **親スコープの変数への書き込み**（シャドウイングなし） | 高 | 🔴 高 |
| **多重ネストブロック**（3階層以上） | 高 | 🔴 高 |
| **while 内でのスコープ** | 高 | 🔴 高 |
| **if/else 両方でのスコープ** | 中 | 🟡 中 |
| **ホイスティングの動作** | 高 | 🔴 高 |
| **ブロック内での複数変数定義** | 低 | 🟢 低 |
| **空のブロック** | 低 | 🟢 低 |

---

## 追加すべきテストケース

### 🔴 優先度: 高

#### 1. 親スコープ変数への書き込み（シャドウイングなし）

**目的**: 子スコープから親スコープの変数を変更できることを確認

```nospace
func: main() {
  let:x;
  x = 1;
  __assert(x == 1);
  if:1{
    x = 2;  # 親の x を変更（シャドウイングなし） #
    __assert(x == 2);
  };
  __assert(x == 2);  # 親の x が変更されている #
}
```

**ファイル名**: `scope_parent_write_001.ns`

---

#### 2. 多重ネストブロック

**目的**: 3階層以上のネストで変数解決が正しく動作することを確認

```nospace
func: main() {
  let:x;
  x = 1;
  if:1{
    let:y;
    y = 2;
    __assert(x == 1);
    if:1{
      let:z;
      z = 3;
      __assert(x == 1);
      __assert(y == 2);
      __assert(z == 3);
      x = 10;  # 2階層上の変数を変更 #
      y = 20;  # 1階層上の変数を変更 #
    };
    __assert(x == 10);
    __assert(y == 20);
    # z はアクセス不可 #
  };
  __assert(x == 10);
  # y はアクセス不可 #
}
```

**ファイル名**: `scope_nested_blocks_001.ns`

---

#### 3. while 内でのスコープ

**目的**: while ブロック内での変数スコープが正しく動作することを確認

```nospace
func: main() {
  let:i;
  let:sum;
  i = 3;
  sum = 0;
  while:i{
    let:temp;
    temp = i;
    sum = sum + temp;
    i = i - 1;
  };
  __assert(sum == 6);  # 3 + 2 + 1 = 6 #
}
```

**ファイル名**: `scope_while_001.ns`

---

#### 4. ホイスティングの動作確認

**目的**: 変数宣言より前に使用できることを確認（巻き上げ）

```nospace
func: main() {
  x = 5;  # 宣言より前に使用 #
  __assert(x == 5);
  let:x;
  __assert(x == 5);
  x = 10;
  __assert(x == 10);
}
```

**ファイル名**: `scope_hoisting_001.ns`

---

### 🟡 優先度: 中

#### 5. if/else 両方でのスコープ

**目的**: if と else の両方で独立したスコープが機能することを確認

```nospace
func: main() {
  let:cond;
  cond = 0;
  if:cond{
    let:x;
    x = 1;
    __assert(x == 1);
  };
  else:{
    let:x;  # else ブロック内で別の x #
    x = 2;
    __assert(x == 2);
  };
  # どちらの x もアクセス不可 #
}
```

**ファイル名**: `scope_if_else_001.ns`

---

#### 6. 多重シャドウイング

**目的**: 同じ変数名を3階層でシャドウイングできることを確認

```nospace
func: main() {
  let:x;
  x = 1;
  if:1{
    let:x;
    x = 2;
    __assert(x == 2);
    if:1{
      let:x;
      x = 3;
      __assert(x == 3);
    };
    __assert(x == 2);
  };
  __assert(x == 1);
}
```

**ファイル名**: `scope_shadow_multi_001.ns`

---

### 🟢 優先度: 低

#### 7. ブロック内での複数変数定義

```nospace
func: main() {
  if:1{
    let:a;
    let:b;
    let:c;
    a = 1;
    b = 2;
    c = 3;
    __assert(a + b + c == 6);
  };
}
```

**ファイル名**: `scope_multiple_vars_001.ns`

---

#### 8. 空のブロック

```nospace
func: main() {
  let:x;
  x = 1;
  if:1{
    # 空のブロック #
  };
  __assert(x == 1);
}
```

**ファイル名**: `scope_empty_block_001.ns`

---

## テスト実装計画

### Phase 1（高優先度）

1. ✅ `disabled_scope_block_var_001` を有効化
2. 🆕 `scope_parent_write_001` を追加
3. 🆕 `scope_nested_blocks_001` を追加
4. 🆕 `scope_while_001` を追加
5. 🆕 `scope_hoisting_001` を追加

### Phase 2（中優先度）

6. 🆕 `scope_if_else_001` を追加
7. 🆕 `scope_shadow_multi_001` を追加

### Phase 3（低優先度）- 必要に応じて

8. 🆕 `scope_multiple_vars_001` を追加
9. 🆕 `scope_empty_block_001` を追加

---

## 実装手順

1. テストケースファイル (`.ns`) を作成
2. 期待値ファイル (`.check.json`) を作成
3. `test-manifest.yaml` にエントリを追加
4. テストを実行して動作確認
5. 必要に応じて修正

---

## 期待される効果

- Phase 1 実装済み機能の動作を包括的に検証
- Phase 2 実装時の回帰テストとして機能
- エッジケースの発見と修正
- 将来の仕様変更時の影響範囲を把握

---

## 備考

- `scope_nested_func_001` は Phase 5 まで無効化のままとする
- 各テストは単一の機能に焦点を当て、シンプルに保つ
- `__trace()` は必要最小限に（デバッグ時のみ）
