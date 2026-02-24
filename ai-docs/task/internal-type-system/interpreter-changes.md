# interpreter の変更設計

## 変更方針

semantic_analyzer で型チェックが完了した後の実行では、void 式の値が変数に代入されたり演算に使われたりすることはない。したがって、interpreter の**実行時動作は基本的に変更不要**。

## 変更対象ファイル

- `src/interpreter/exec.rs` — 最小限の調整のみ

## 詳細

### ExpressionFlow の変更

`ExpressionFlow` は `Value(i64)` と `Jump(Flow)` の2バリアントを持つ。void 型の式も実行時には内部的に `Value(0)` を返す。型チェック済みのため、この値が不正に使用されることはない。

**変更なし**: `ExpressionFlow` に `Void` バリアントを追加する必要はない。

### While 式

現状: `ExpressionFlow::Value(0)` を返す → **変更なし**

型チェックにより `x = while: ...` のようなコードは semantic_analyzer で拒否される。interpreter に到達しない。

### If 式

現状:
- else あり: 分岐先ブロックの最後の式の値を返す → **変更なし**
- else なし: 空ブロック（`last_value = 0`）を返す → **変更なし**

型チェックにより else なし if の値使用は拒否される。

### Block 式

現状: 最後の式の値を返す。空ブロックは `0` → **変更なし**

型チェックにより空ブロックの値使用は拒否される。

### 関数呼び出し

現状: `Flow::Proceed`（return なし）→ `ExpressionFlow::Value(0)` → **変更なし**

void 関数の返り値使用は型チェックで拒否される。

## まとめ

interpreter は型チェック済みコードの実行のみを担当するため、実行時の型エラーを検出する必要がない。全ての型エラーは semantic_analyzer で検出される。

interpreter に必要な変更:
- **なし**（既存のコードがそのまま動作する）
