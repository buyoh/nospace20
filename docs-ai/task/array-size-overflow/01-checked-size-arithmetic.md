# 01: サイズ計算の checked 化（案 A）

配列サイズの変換・スロット数の累積・struct サイズ計算・WS アドレスへの変換を checked にし、収まらない場合はエラーを返す。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

サイズの値は次の順に流れる。各段で「次の段の型に収まること」を検査する。

```
パーサ            Token::Number(i64) → usize           (1) usize::try_from
意味解析          要素数 × 要素サイズ、フィールドの累積、
                  スコープ内の変数の累積 → usize         (2) checked_mul / checked_add
compiler_ws       関数フレーム合計・グローバル領域合計
                  → i64（WS のアドレス）                  (3) try_from + checked_add
```

(3) で「最大値」（関数フレームの合計サイズ、`GLOBAL_PTR + グローバル領域のサイズ`）を一度検査すれば、その内側の個々のオフセット（`GLOBAL_PTR + offset` など）は最大値以下なので、個別の検査は不要。

例外として `enter_block_scope` の `next_var_offset` は、どの「最大値」にも含まれないブロックを通る経路がある（ルートレベルのブロック、関数内 static 初期化子のブロック。下記 (3) の表）。このため `enter_block_scope` 自体を checked にする。

### (1) パーサ: 配列サイズの変換

| 位置 | 現状 | 変更 |
|---|---|---|
| `src/tree_parser/statement/mod.rs` `parse_type_spec`（`Some((Token::Number(n), _)) if *n > 0 => *n as usize`） | wasm32 で切り捨て | `usize::try_from(*n)` が失敗したら parse error `array size too large` |
| `src/tree_parser/expression/mod.rs` `parse_type_spec`（同上） | 同上 | 同上 |
| `src/tree_parser/statement/mod.rs` struct フィールドの `name[N]`（`StructFieldDecl.array_size`、1324 行付近） | 同上 | 同上 |
| `src/tree_parser/statement/mod.rs:867` `parse_array_list_init` の `init_values.len() > explicit_size as usize` | wasm32 で `explicit_size` が切り捨てられ、正しいプログラムが `too many initializers` になる | `i64` 同士の比較にする（`init_values.len()` を `i64::try_from`） |

- `parse_type_spec` は [tree-parser-dedup](../tree-parser-dedup/README.md) で 1 箇所に集約される予定。先に dedup が終わっていれば 1 箇所の修正で済む。順序は問わない
- 変数宣言の `[N]`（`parse_array_size`）は `Option<i64>` のまま意味解析に渡しているので、(2) の `mod.rs:694` で変換する

### (2) 意味解析

| 位置 | 変更 | エラー |
|---|---|---|
| `mod.rs:694` `array_size.map(\|n\| n as usize)` | `usize::try_from` | `semantic error: array size of '{name}' is too large` |
| `mod.rs:1016` `inner_size * size` | `checked_mul`。失敗時は `errors` に積んで `0` を返す（`value_type_size` の既存の失敗表現。呼び出し元 `mod.rs:929` は `size == 0` で中断する） | `semantic error: size of a field in '{struct}' is too large` |
| `mod.rs:940` `offset += size` | `checked_add`。失敗時は `errors` に積んで中断（`visiting.remove(name); return;`） | `semantic error: size of struct '{struct}' is too large` |
| `mod.rs:212` `slot_index += var.array_size.unwrap_or(1)` | `checked_add`。失敗時は `Err` を返す | `semantic error: total size of variables in a scope is too large` |
| `scope.rs:710`（`build()` 内の同じ累積） | `checked_add(...).expect(...)`。#212 で検証済みであることをコメントに記載 | なし（不変条件違反は panic） |

- `scope.rs:710` について: `build()` は `Scope` を返す関数で、`Result` にすると呼び出し元（`statement.rs` / `expression.rs` の 10 箇所）すべてに影響する。`build()` を呼ぶ `ScopeBuilder` が必ず `mod.rs:212` の累積を通ることを実装時に確認し、通らない経路があれば `build()` を `Result` にする
- `mod.rs:212` と `scope.rs:710` の重複は、スロット配置を計算する関数を 1 つにまとめて解消してよい（例: `fn compute_slot_layout(variables, names) -> Option<(BTreeMap<String, usize>, BTreeMap<String, usize>, usize)>`）。必須ではない
- `collect_struct_declarations` はエラーを `errors` に集める方式（複数報告）なので、struct のエラーもそれに合わせる
  - サイズが溢れた struct は `resolved` に入らないため、別の struct から参照されると再解決され、同じエラーが二重に積まれうる。既存の void フィールドのエラーと同じ性質で、テストの `contains` 判定にも影響しないので許容する
- エラー位置: struct のエラーは既存の struct エラーと同様に位置なし（`code_parse_error!(msg)`）でよい。変数のエラーは `located_stat.location.start` を付ける。`mod.rs:212` の累積は位置を持たない（ループが `scope.variables` 単位）ので、溢れた変数の名前をメッセージに含める

### (3) compiler_ws

> 注（2026-10-19）: [ws-root-block-variable-address](../../done-task/ws-root-block-variable-address/README.md) で以下の位置が移動・変更された。行番号は同タスク前のもの。
> - `calculate_total_variable_count`・`count_nested_vars_*` は `src/compiler_ws/block_var_count.rs` に移動
> - `compute_static_var_offsets` は削除され、`src/compiler_ws/global_layout.rs` の `GlobalLayout::compute` に統合。ルート初期化文のブロック変数領域・関数ごとの static 初期化子のフレーム相当領域（`StaticInitFrame`）の合計もここで計算する。`global_heap_size()` は `GlobalLayout::total_size()` を返す。検査は `GlobalLayout::compute`（`compile_with_options` から呼ばれる）で行えばよい
> - `enter_function_for_static_init` は `next_var_offset = frame.block_base` になり、`func_scope_var_count as i64` は無くなった
> - 下表 `enter_block_scope` 行の (a)(b) の経路は、上記の予約領域に数えられるようになったため、`GlobalLayout::compute` の合計の検査で先に捕まる（`enter_block_scope` 側の検査は不要になる可能性がある）

| 位置 | 現状 | 変更 |
|---|---|---|
| `src/compiler_ws/statement.rs:418-` `calculate_total_variable_count` ほか `count_nested_vars_*` | `usize` の `+` / `sum()` | 戻り値を `Option<usize>` にし、`checked_add` で合計。`sum()` は `try_fold` に |
| `src/compiler_ws/context.rs:148` `local_heap_size: total_var_count as i64` | 未検査の変換 | `i64::try_from`。`enter_function` / `enter_function_for_static_init` の呼び出し元（`statement.rs:27`、`statement.rs:377`）で検査して `CompileError` を返し、`enter_function` には `i64` を渡す |
| `src/compiler_ws/context.rs:28-41` `compute_static_var_offsets` | `as i64` と `+=` | `i64::try_from` + `checked_add`。戻り値を `Result` にする |
| `src/compiler_ws/context.rs:198` `global_heap_size` | `variable_count as i64 + static_var_total_size` | `CodeGenContext::new` で `GLOBAL_PTR + variable_count + static_var_total_size` が `i64` に収まることを検査し、結果を保持する。`global_heap_size()` は保持した値を返す |
| `context.rs:155` / `187` `next_var_offset: func_scope_var_count as i64` | 同上 | 関数フレーム合計（≥ `func_scope_var_count`）が検査済みなので、`i64::try_from(...).expect(...)` かコメントで検証済みを明記 |
| `context.rs:324` `enter_block_scope` の `next_var_offset +=` | 同上 | `i64::try_from` + `checked_add` にし、`Result<(), CompileError>` を返す。呼び出し元は `statement.rs:70`・`253` の 2 箇所。関数内のブロックは関数フレームの検査で先に捕まるが、次の 2 経路はどの合計にも含まれないため、ここでしか検出できない: (a) ルートレベルのブロック・if/while/for（グローバルの `CodeGenContext` で `next_var_offset` が 0 から積まれる）、(b) 関数内 static 初期化子に含まれるブロック（`static_init_statements` は `calculate_total_variable_count` に数えられない）。実測でどちらも現状 `context.rs:324` で panic する |
| `alloc_runtime/bump.rs:30`、`fsba.rs:647` `GLOBAL_PTR + global_heap_size` | 同上 | `CodeGenContext::new` の検査で保証済み。コメントのみ |

- `CodeGenContext::new` は現在 `Self` を返す。`Result<Self, CompileError>` にするか、検査を `compile_with_*`（`compiler_ws/mod.rs`）側で先に行うかは実装時に決める。後者の方が変更が小さい
- エラー種別は既存の `CompileErrorKind::InvalidOperation(String)` を使う（例: `total size of global and static variables exceeds the address space`）。専用の種別は追加しない
- 関数フレーム・グローバル領域のエラーは、関数宣言の位置（グローバル領域は位置なし）を `CompileError::with_location` で付ける

## バグ修正コメント

AGENTS.md の規約に従い、checked 化した各箇所（`mod.rs:212`・`694`・`940`・`1016`、パーサの `try_from`、`context.rs` の各箇所、`calculate_total_variable_count`）に、原因（未チェックの `+` / `*` / `as` で debug は panic、release は wrap して誤ったスロット配置のコードを出力）と解決策（checked にしてエラーを返す）をコメントで書く。

## 対象外

- 実行時のアドレス計算（`GLOBAL_PTR + offset + index`、フレームポインタ + オフセット、再帰呼び出しでのフレームポインタの増加）。WS の算術は wrap する仕様（[fix-parser-loop-ws-number-overflow](../../done-task/fix-parser-loop-ws-number-overflow/) で決定済み）で、範囲外の添字と同じ扱い。詳細と残る懸念は [02](02-size-limit-option-b.md) の「案 B を実施しない場合に残る懸念」
- 直接実行インタプリタ（[README](README.md) の設計・方針を参照）

## テスト

`resources/tests/fails/compile/<name>.ns` と `<name>.check.json`（`{"type": "compile_error", "contains": [...]}`）を追加し、`resources/tests/test-manifest.yaml` に `type: compile_error` で登録する。`exclude_targets` は指定しない（`compile_error` は意味解析と WS コンパイルを行う型で、インタプリタを除外するとテスト関数が生成されなくなる。`src_build/nospace_tests.rs:352-366`）。

`compile_error` は意味解析エラーでもコンパイルエラーでも keyword が一致すれば通るため、フェーズの区別は keyword で行う。意味解析のエラーは `too large`、compiler_ws のエラーは `exceeds the address space` と、文言をフェーズごとに分ける。

| 名前 | 内容 | 期待 |
|---|---|---|
| `array_size_overflow_vars_001` | `let: a[9223372036854775807], b[9223372036854775807], c[9223372036854775807];` | `compile_error`（意味解析）`too large` |
| `array_size_overflow_struct_001` | `struct: S (d@int[4611686018427387904]); struct: T (x@S[8]);` を使う | `compile_error`（意味解析）`too large` |
| `array_size_overflow_struct_offset_001` | `struct: S (a[9223372036854775807], b[9223372036854775807], c[9223372036854775807]);` | `compile_error`（意味解析）`too large` |
| `array_size_overflow_frame_001` | 関数内で `let: a[9223372036854775807];` と、ネストしたブロック `{ let: b[9223372036854775807]; }` を置く（各スコープは `usize` に収まるが、フレーム合計が `i64` を超える） | `compile_error`（compiler_ws）`exceeds the address space` |
| `array_size_overflow_global_001` | グローバル変数 `let: a[9223372036854775807];`（`GLOBAL_PTR` を足すと超える） | 同上 |
| `array_size_overflow_root_block_001` | `{ let: b[9223372036854775807]; b[0]=5; }; { let: c[9223372036854775807]; c[0]=6; }; func: __main(){}`（ルートレベルのブロック） | 同上 |
| `array_size_overflow_static_init_001` | `func: f(){ static: x({ let: t[9223372036854775807]; 1; }); static: y({ let: u[9223372036854775807]; 1; }); }` と `__main`（static 初期化子のブロック） | 同上 |

- 正常系: `let: a[9223372036854775000];` 単独の関数が compile できること（巨大でも表現できるサイズは受理する方針の確認）。manifest に compile のみの型は無く、`success` にすると直接実行インタプリタが `capacity overflow` で panic するため、unit テストにする。置き場所は `src/compiler_ws/` の既存テスト（`compile_with_options` が `Ok` を返すことを確認）
- unit テスト: `value_type_size` の `checked_mul` 失敗、`calculate_total_variable_count` の合計失敗は fails テストで十分にカバーされるので追加しない

## ドキュメント

- `docs/spec.md` の配列・struct の節に「配列・struct・変数の合計サイズがアドレス空間（64bit 符号付き整数）を超える場合はコンパイルエラー」と追記
- `VERSION.md` の WIP に「Semantic analyzer / compiler: Report an error instead of panicking or silently wrapping when the size of arrays, structs or variables overflows」
