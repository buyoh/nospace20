# 03: WhitespaceVM の算術演算を wrapping に統一し、処理系の挙動を spec.md の Appendix に記載する

`WhitespaceVM` の Add / Sub / Mul / Div / Mod を、ツリー走査インタプリタ・NospaceVM・定数畳み込みと同じ 64 ビット 2 の補数の wrapping 演算に揃え、ビルドプロファイルに依存した panic / wrap をなくす。

作成日: 2026-10-18
ステータス: 完了

## 背景・動機

[code-review-2026-10/03](../../task/code-review-2026-10/03-ws-vm-arithmetic-overflow.md) 参照。採用案は **A（wrapping 統一）**。あわせて、レビュー記録の案 C のうち「spec.md への記載」と「本リポジトリの実行系間の一致を確認する large テスト」までを本タスクで行う。ただし整数の範囲とオーバーフローは言語仕様上は実装依存とし、64 ビット wrapping は処理系の挙動として spec.md の Appendix に記載する（[spec.md への記載](#specmd-への記載)）。

- `src/whitespace/interpreter.rs` の `Instruction::Add` / `Sub` / `Mul` は素の `+` `-` `*` を使っており、debug ビルドで panic、release ビルドで wrap する
- `Div` / `Mod` は 0 除算のみチェックしており、`i64::MIN / -1`・`i64::MIN % -1` はビルド設定に関係なく panic する。`panic = "abort"` のため CLI は abort、WASM は trap する

### 案 A を採用する理由

| 観点 | A: wrapping | B: checked + エラー |
|---|---|---|
| 他の実行系との一致 | `base/pure_eval.rs`（インタプリタ・NospaceVM・定数畳み込み・constexpr が共用）が wrapping なので一致する | インタプリタ側は wrapping のままなので不一致が残る |
| 変更範囲 | `whitespace/interpreter.rs` のみ | エラー型・whitespace20・WASM の表示も変更 |
| 最適化との関係 | 定数畳み込みの結果と実行時の結果が一致する | 畳み込まれた式はエラーにならず、畳み込まれない式はエラーになる |

wsc（多倍長整数）とは結果が異なるままになるが、言語仕様上オーバーフロー時の挙動は実装依存なので仕様違反ではない。差異は spec.md の Appendix に記載する。

## 設計・方針

### VM の変更

`src/whitespace/interpreter.rs` の算術演算 5 命令を、`base::pure_eval` と同じ wrapping 演算にする。

| 命令 | 変更前 | 変更後 |
|---|---|---|
| Add | `a + b` | `a.wrapping_add(b)` |
| Sub | `a - b` | `a.wrapping_sub(b)` |
| Mul | `a * b` | `a.wrapping_mul(b)` |
| Div | `a / b`（0 除算チェックあり） | `a.wrapping_div(b)`（0 除算チェックは維持） |
| Mod | `a % b`（0 除算チェックあり） | `a.wrapping_rem(b)`（0 除算チェックは維持） |

- 結果: `i64::MAX + 1 = i64::MIN`、`i64::MIN / -1 = i64::MIN`、`i64::MIN % -1 = 0`
- 除算は 0 方向への切り捨て、剰余の符号は被除数に従う（Rust の `/` `%` と同じで、現状から変化なし）
- `pure_eval::eval_binary_pure` を直接呼ぶことも考えられるが、`pure_eval` は `tree_parser::Operator2` に依存しており、`whitespace` モジュールを nospace のフロントエンドから独立させている現在の依存方向（`whitespace` は `base::ws_types` のみに依存）を崩すため採用しない。5 行の wrapping 呼び出しを直接書く
- バグ修正コメント（Add の直前などにまとめて 1 箇所）:

```rust
// === 算術演算 ===
// バグ修正: 従来は素の演算子を使っていたため、オーバーフロー時に debug ビルドでは panic、
// release ビルドでは wrap とビルド設定で挙動が変わり、i64::MIN / -1 と i64::MIN % -1 は
// 常に panic（panic = "abort" のため CLI は abort、WASM は trap）していた。
// 解決策: 本処理系の挙動（docs/spec.md「Appendix」）として定めたとおり、
// base::pure_eval と同じ 64 ビット 2 の補数の wrapping 演算に統一する。
// （言語仕様上、オーバーフロー時の挙動は実装依存）
```

### spec.md への記載

nospace 言語としては、整数は「十分大きな整数型」程度の厳密性でしか定義しない。実行環境である Whitespace 処理系が実装依存（多倍長・64 ビット・32 ビットなど）であるため、整数の範囲とオーバーフロー時の挙動は**実装依存**とする。本タスクで決める 64 ビット wrapping は言語仕様ではなく、本リポジトリの処理系（nospace20 / whitespace20）の挙動である。

そのため spec.md への記載は次の 2 か所に分ける。

#### (1) 言語仕様: 「### int 型」節に追記

`docs/spec.md` の「## 暗黙的型システム」→「### int 型」に以下を追記する（言語仕様として書くのはこれだけ）。

- int 型は十分大きな範囲を持つ符号付き整数である
- 表現可能な範囲、範囲外となった場合（オーバーフロー）の挙動は実装依存とする

#### (2) 処理系の挙動: 末尾に「## Appendix」を新設

`docs/spec.md` の末尾に「## Appendix」節を追加し、その下に「### nospace20 処理系の整数演算」を置く。冒頭で「この節は言語仕様ではなく、本リポジトリの処理系（インタプリタ `nospace20 run`、NospaceVM、Whitespace コンパイラの定数畳み込み、Whitespace インタプリタ `whitespace20`）の挙動を記述する」と明記する。記載内容:

- 整数は 64 ビット符号付き整数（-9223372036854775808 〜 9223372036854775807）として扱う
- 整数リテラルは `9223372036854775807` を超えると字句解析エラー（`integer literal overflow`）。最小値は `-9223372036854775807 - 1` と書く
- `+` `-` `*` と単項 `-`、および複合代入（`+=` `-=` `*=` `/=` `%=`）の結果が範囲外になる場合は、2 の補数で折り返す（wrapping）。例: `9223372036854775807 + 1` は `-9223372036854775808`
- `/` は 0 方向への切り捨て、`%` の結果の符号は左辺に従う。`-9223372036854775808 / -1` は `-9223372036854775808`、`-9223372036854775808 % -1` は `0`
- `whitespace20` は 64 ビット範囲外の数値リテラルを含む Whitespace コードをパースエラー（`NumberOverflow`）とする（[02](02-ws-parser-number-and-label.md)）
- Whitespace にコンパイルしたプログラムを外部の Whitespace 処理系（多倍長整数の wsc など）で実行した場合、範囲外となる演算の結果は上記と一致しない（言語仕様上は実装依存のため許容される）
- 既知の制限: 範囲の境界付近の値どうしの比較は、Whitespace 出力および一部の最適化で誤った結果になる場合がある（[09](../../task/code-review-2026-10/09-condition-opt-overflow-semantics.md) で修正予定。修正後にこの項目を削除する）

0 除算時の挙動（インタプリタは panic、WS VM はエラー）は実行系ごとに異なり、統一は [05-runtime-error-panic.md](../../task/code-review-2026-10/05-runtime-error-panic.md) の範囲なので、本タスクでは Appendix に記載しない。

### 範囲外（関連タスクに委ねるもの）

- 比較演算の最適化・コード生成が差分（`a - b < 0`）で比較するため、wrapping 仕様の下でもオーバーフロー時に比較結果が変わる問題は [09](../../task/code-review-2026-10/09-condition-opt-overflow-semantics.md)。本タスクの large テストには `i64::MAX` / `i64::MIN` 近傍の比較を含めない
- `Copy` 命令の `1 + n.0.unsigned_abs() as usize` は wasm32（usize = 32 ビット）で切り捨て・オーバーフローの可能性がある。WS VM 命令の不整合として [15](../../task/code-review-2026-10/15-ws-vm-instruction-inconsistency.md) で扱う

## タスクステップ・チェックリスト

1. [x] `interpreter.rs` の Add / Sub / Mul / Div / Mod を wrapping 演算に変更（バグ修正コメント付き）
2. [x] ユニットテスト追加（`src/whitespace/interpreter_tests.rs`）
3. [x] large テスト追加（`resources/tests/passes/`、`test-manifest.yaml`）
4. [x] `docs/spec.md` の「### int 型」に範囲・オーバーフローが実装依存である旨を追記
5. [x] `docs/spec.md` 末尾に「## Appendix」→「### nospace20 処理系の整数演算」を追加
6. [x] [09](../../task/code-review-2026-10/09-condition-opt-overflow-semantics.md) の「方針決定」を「言語仕様上は実装依存、本処理系は wrapping に確定（本タスク）」と更新
7. [x] `VERSION.md` の WIP セクションに変更内容を追記（例: `whitespace20: Arithmetic overflow now wraps (64-bit two's complement) instead of panicking`）

### ステップ 2: ユニットテスト

`from_instructions` で命令列を直接組み（末尾に必ず `Instruction::Exit` を置く。無いと `ProgramCounterOutOfBounds` になる）、`vm.run()` が `Complete` し、`data_stack()` が期待値になることを確認する（既存の `test_push_and_add` と同じ形式）。

| テスト | 命令列 | 期待スタック |
|---|---|---|
| Add オーバーフロー | `push i64::MAX, push 1, add` | `[i64::MIN]` |
| Sub オーバーフロー | `push i64::MIN, push 1, sub` | `[i64::MAX]` |
| Mul オーバーフロー | `push i64::MAX, push 2, mul` | `[-2]` |
| Div `MIN / -1` | `push i64::MIN, push -1, div` | `[i64::MIN]` |
| Mod `MIN % -1` | `push i64::MIN, push -1, mod` | `[0]` |
| 0 除算は従来どおり | `push 1, push 0, div` | `StepResult::Error(DivisionByZero)` |

### ステップ 3: large テスト

`resources/tests/passes/int_overflow_wrapping_001.ns` を追加し、本リポジトリの全実行系（インタプリタ・NospaceVM・whitespace-self・opt-all）で Appendix に記載した同じ出力になることを確認する。言語仕様上は実装依存の挙動なので、テストの目的は「処理系の挙動の固定」であることを `.ns` 冒頭のコメントに記載する。

```
func: __main() {
  let: max(9223372036854775807);
  let: min(-9223372036854775807 - 1);
  let: m1(0 - 1);
  __puti(max + 1); __putc('\n');
  __puti(min - 1); __putc('\n');
  __puti(max * 2); __putc('\n');
  __puti(min / m1); __putc('\n');
  __puti(min % m1); __putc('\n');
}
```

（変数宣言・出力の構文は実装時に spec.md / 既存テストに合わせて調整する。定数畳み込みで消えないよう、オペランドは変数経由にする。ただし opt-all では畳み込まれる可能性があり、その場合も `pure_eval` の wrapping で同じ結果になるので問題ない。）

- 期待出力: `-9223372036854775808` / `9223372036854775807` / `-2` / `-9223372036854775808` / `0`
- `int_overflow_wrapping_001.check.json` は `{"type": "success_io", "stdin": "", "stdout": "..."}` 形式。`test-manifest.yaml` に `type: success_io` で登録し、`exclude_targets: [whitespace]` を指定する（`whitespace` ターゲットは多倍長の wsc で実行するため結果が異なる）。除外理由を `comment` に記載する
