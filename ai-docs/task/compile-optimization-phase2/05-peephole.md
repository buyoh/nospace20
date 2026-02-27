# ピープホール最適化 (`peephole`)

## 概要

生成された Whitespace 命令列に対して、局所的なパターンマッチで冗長命令を除去・簡約する後処理パス。中間表現レベルではなく、最終的な `WsProgram`（命令列）に対して適用する。

## 背景

他の最適化パスは中間表現 (`Scope` / `ExecExpression`) を対象とするが、ピープホール最適化は出力命令列を直接操作する。複数のパスの相互作用で生じる冗長パターンや、中間表現では検出困難な命令レベルの非効率を排除できる。

## 検出・簡約パターン

### パターン 1: Push + Discard の除去

```
Push(x)
Discard
→ (削除)
```

空ブロックや void 関数呼び出しの式文で発生。void 関数が `Push(0)` を返し、直後に `Discard` される。

### パターン 2: Duplicate + Discard の除去

```
Duplicate
Discard
→ (削除)
```

### パターン 3: Push(0) + Add の除去

```
Push(0)
Add
→ (削除)
```

ローカル変数のオフセットが 0 の場合、`Push(0)` + `Add` が名目上のアドレス計算として残る。

### パターン 4: Jump の短絡

```
Jump(L1)
...
Label(L1)
Jump(L2)
→
Jump(L2)
...
Label(L1)
Jump(L2)
```

連鎖的なジャンプを直接化する。条件式最適化後の `ConditionMode::Zero` / `Negative` で発生しうる。

### パターン 5: 到達不能コードの除去

```
Jump(L1)
Push(x)      ← 到達不能
Add           ← 到達不能
Label(L2)    ← ラベルがあれば到達可能になる
```

無条件ジャンプ・Return・Exit の後、次のラベルまでの命令を除去。

### パターン 6: 連続同値 Push の簡約

```
Push(x)
Push(x)
→
Push(x)
Duplicate
```

Whitespace では Duplicate は Push より短いエンコーディングになる場合がある（数値のビット長による）。

## 設計

### 実装レベル

`WsProgram` に対するポストプロセスとして実装する。

```rust
// src/compiler_ws/peephole.rs (新規)
pub fn optimize(prog: &mut WsProgram) {
    // 複数パスで固定点に到達するまで繰り返す
    loop {
        let changed = apply_patterns(prog);
        if !changed { break; }
    }
}
```

### パイプラインの位置

```
Scope → Compiler WS → WsProgram → [Peephole] → エンコード → Whitespace 出力
```

中間表現の最適化（Phase 1 パス群）の**後**、Whitespace エンコードの**前**に適用。

### 変更対象ファイル

| ファイル | 変更内容 |
|---|---|
| `src/compiler_ws/peephole.rs` (新規) | ピープホール最適化ロジック |
| `src/compiler_ws/mod.rs` | パイプラインへの統合 |
| `src/compiler_ws/program.rs` | `WsProgram` にパターンマッチ用のインターフェース追加（必要に応じて） |

### 最適化オプション

`--opt peephole` として個別に制御可能にする。`--opt all` に含める。

## 効果

- 個々の削減量は小さい（1〜3命令）が、出現頻度が高い
- 他の最適化パスによって生じた冗長も回収できる安全網として機能
- エンコード後のバイナリサイズも削減

## ドキュメント更新

- `docs/optimize.md` に `peephole` パスの説明セクションを追加
- パス一覧テーブルへの追記
- パスの実行順序への追記（最終段として記載）

## 難易度: 中

パターンマッチは単純だが、ジャンプ先の更新やラベルの参照カウント管理が必要な場合がある（パターン 4, 5）。パターン 1〜3 から段階的に実装するのが望ましい。
