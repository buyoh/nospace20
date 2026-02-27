# 末尾呼出し最適化 (`tail-call`)

## 概要

関数の最後の操作が関数呼び出しである場合（末尾位置）、Call + Return の代わりに Jump を使用し、スタックフレームを再利用する。再帰関数のスタックオーバーフローを防止し、実行ステップ数を削減する。

## 背景

fibonacci のプロファイルデータでは、Call/Return が全実行ステップの 12.4% を占めている。再帰的なプログラムでは関数呼び出しオーバーヘッドが支配的であり、末尾呼出し最適化の効果が大きい。

### 対象パターン

```nospace
# 末尾再帰（自己呼出し）
func: fact_iter(n, acc) {
  if: n <= 1 {
    return: acc;
  } else: {
    return: fact_iter(n - 1, acc * n);  # ← 末尾位置
  };
}

# 末尾呼出し（他関数呼出し）
func: dispatch(x) {
  if: x == 0 {
    return: handler_a();  # ← 末尾位置
  } else: {
    return: handler_b();  # ← 末尾位置
  };
}
```

### 非対象パターン

```nospace
func: fact(n) {
  if: n <= 1 {
    return: 1;
  } else: {
    return: n * fact(n - 1);  # ← n * が後にあるため末尾位置ではない
  };
}
```

## 設計

### 末尾位置の定義

以下の位置にある関数呼び出しが末尾呼出しとなる：

1. `return: f(args);` — return 文の式が直接関数呼び出し
2. 関数本体の最後の式が関数呼び出し（implicit return）

### 自己末尾再帰の変換（フェーズ 1）

最もシンプルかつ効果的なケース。関数が自分自身を末尾位置で呼び出す場合、引数を更新してループ先頭に Jump する。

```
# 最適化前
return: f(new_args)
→
eval(new_args)
Copy args to local vars
Jump(function_entry)   # Call/Return の代わりにジャンプ

# 最適化後のコード構造
Label(function_entry)
setup_frame
Label(loop_point)       # ← ここにジャンプ
function_body
# 末尾再帰の場所:
update_args             # 新しい引数をローカル変数に上書き
Jump(loop_point)        # Call/Return なし
```

### 一般的な末尾呼出し最適化（フェーズ 2）

他の関数への末尾呼出し。現在のフレームを解放してからジャンプする必要があり、Whitespace のスタックベース実行モデルでは実装が複雑。

→ フェーズ 1（自己末尾再帰のみ）の実装を優先。

### 最適化パスの位置

中間表現レベル（`Scope` → `Scope`）で実装するのが望ましい。`return: self_call(args)` を `WhileTailRecursion` のような特殊な文に変換する。

### 変更対象ファイル

| ファイル | 変更内容 |
|---|---|
| `src/optimizer/tail_call.rs` (新規) | 末尾再帰の検出と変換 |
| `src/optimizer/mod.rs` | パスの登録 |
| `src/semantic_analyzer/types.rs` | 必要に応じて新しいバリアント追加 |
| `src/compiler_ws/statement.rs` | 末尾再帰ループのコード生成 |
| `src/interpreter/exec.rs` | インタプリタ対応 |

## 効果

- 再帰関数でのスタックオーバーフロー防止
- fibonacci のような再帰プログラムで Call/Return 削減（最大 12% のステップ削減に寄与）
- ループに変換されるため、メモリ使用量も大幅削減

## 難易度: 高

- 末尾位置の正確な判定が必要（if/else のネスト、ブロック式など）
- フレーム再利用時の引数更新順序に注意（同じスロットへの上書きで値が壊れないようにする）
- Whitespace のスタックベースモデルでは一般的な末尾呼出しの実装が困難

## 優先度: 低

再帰を多用するプログラムでは効果大だが、実装の複雑さが高い。フェーズ 1（自己末尾再帰のみ）を検討対象とする。
