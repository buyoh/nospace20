# 02: `__clog` を debug 拡張 API `-13` として実装する

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 背景・動機

- 現行のツリー走査インタプリタと NospaceVM は、`__clog(n)` でホストの stdout に `__clog: n` を `println!` している
- WS コンパイラは、debug 拡張の有無にかかわらず `__clog` を noop にしている（`compiler_ws/expression.rs` の `Clog => generate_builtin_debug_noop`）。このままでは、CLI run を WS に置き換えた時点で `__clog` の出力が消える。`docs/spec.md` の例は `__clog` を多用しているため、影響が大きい

## 設計・方針

### 拡張 API の追加

| アドレス | API | 拡張 | 動作 |
|---|---|---|---|
| `-13` | `__clog(n)` | `debug` | 値 `n` を `__clog: {n}\n` として **clog 用の writer** に書く。ヒープには書き込まない |

既存の `-10`（`__trace`）〜`-12`（`__assert_not`）と同じ「負アドレスへの store を API 呼び出しとして解釈する」方式である。`heap_layout::EXT_CLOG_ADDR = -13` を追加する（`-13` は未使用であることを確認済み）。

### debug 拡張が無効なとき

- `-13` への store は、**普通のメモリ書き込み**として扱う。何もトリガーしない（`-10`〜`-12` と同じ）
- 理由: 今後、メモリ書き込みの実装を高速化する可能性がある。書き込みが副作用をトリガーしうると、高速化した経路でもアドレスを判定し続ける必要がある。トリガーできないほうが実装が容易になる。debug 拡張のフック判定は、`debug_ext` が有効なときだけ通る分岐の中に閉じ込める（現行の `heap_store` の `if self.debug_ext { match addr { ... } }` の構造を保つ）

### 出力先: clog 用の writer

- `WhitespaceVM::with_clog_writer(Box<dyn Write>)` で設定する。未設定なら、出力を捨てる（`-13` への store はヒープに書かず、何も出力しない）
- プログラムの stdout（`with_io` / `with_stdout` の writer）とは分ける。理由:
  - success_io の `builtins/debug_clog_001`（期待 stdout `"10"`）と `builtins/debug_return_001`（`"7"`）は、debug 拡張ありの `_ws_self` 系でも実行される。stdout に混ぜると期待値が壊れる
  - WASM で `["debug"]` を渡している既存の利用者の `flush_stdout` に、`__clog:` の行が混ざってしまう
- 書き込みエラーは無視する（実行を止めない）。`__clog` はデバッグ用の補助出力であり、stderr のパイプが切れた（例: `2>&1 | head`）だけでプログラムの実行と stdout の出力を止めるのは過剰なため。プログラムの stdout の書き込みエラーは、従来どおり `IoError` で止める
- 出力順: clog writer に書く前に、プログラムの stdout を flush する。CLI では clog が stderr、プログラムの出力が stdout と別のストリームになるため、flush しないと端末で見たときに前後する。flush は `__clog` のときだけ行うので、通常の実行速度には影響しない
- 利用側の設定:
  - `whitespace20`: debug 拡張が有効なとき、clog writer に stderr を設定する
  - `nospace20 --mode run`: retire-direct-interpreter で stderr を設定する
  - WASM: 03 で扱う

### コンパイラ

- debug 拡張が有効なら `generate_builtin_debug_store(ctx, args, EXT_CLOG_ADDR)`、無効なら現行どおり `generate_builtin_debug_noop`
- 戻り値は `generate_builtin_debug_store` の仕様に従い、引数の値 `n` を返す（`__trace` と同じ）。インタプリタの `__clog` も `n` を返すので一致する。`debug_return_001`（`x = __clog(7); __puti(x);` の期待値 `"7"`）は、この前提で通る

### 入力命令との関係

[ws-vm-input-via-heap-store.md](../ws-vm-input-via-heap-store.md) の実施後は、入力命令の書き込みも `heap_store` を通る。debug 拡張が有効なら、`-13` への入力でも `__clog` が発火する（`-10`〜`-12` と同じ扱い）。コンパイラが入力の格納先に負のアドレスを使うことはないので、nospace のプログラムには影響しない。同タスクの記述（「-10〜-12 のフックが効く」）は、本タスクの実装時に「-10〜-13」に更新する

## テスト

- コンパイラのユニットテスト: debug 拡張ありで、`__clog` が `-13` への store を生成する。なしでは noop
- VM のユニットテスト:
  - debug 拡張あり・clog writer ありで、`-13` への store が writer に `__clog: n\n` を書き、stdout とヒープは変わらない
  - debug 拡張あり・clog writer なしで、何も出力せず、ヒープも変わらない
  - debug 拡張なしで、`-13` への store が普通のヒープ書き込みになる（`heap()[-13] == n`）
- `debug_clog_001` / `debug_return_001` の `_ws_self` 系が、引き続き通ること
- `fails/compile/builtin_arg_too_many_001`（`__clog(1, 2)` を compile error とする）が、引き続き通ること

## チェックリスト

1. [ ] `EXT_CLOG_ADDR` とコンパイラの分岐。`resources/tests/test-manifest.yaml` の `debug_clog_001` のコメント「ignored in Whitespace」を実態に合わせて直す
2. [ ] VM の `-13` フックと `with_clog_writer`
3. [ ] `whitespace20` で clog writer に stderr を設定
4. [ ] テスト
5. [ ] `docs/spec-whitespace.md` の拡張 API 表に `-13`、`docs/spec.md` の `__clog` の説明（出力先は処理系依存で、本処理系は stderr）
