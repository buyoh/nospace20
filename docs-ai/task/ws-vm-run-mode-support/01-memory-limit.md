# 01: WhitespaceVM が使うメモリの上限

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 背景・動機

- nospace の無限再帰（例: `func: f() { return: f(); }`）を WhitespaceVM で実行すると、止まらずにメモリを使い続ける。CLI では OOM で落ち、WASM ではタブが落ちる。NospaceVM は、フレーム数上限で `StackOverflow` エラーにしていた
- Whitespace の仕様にコールスタックはない。あるのは計算用のスタック（データスタック）とヒープだけで、`Call` / `Return` の戻り先は処理系の内部状態にすぎない。nospace の擬似的なコールスタック（ローカル変数のフレーム）は、コンパイラがヒープ（`LOCAL_HEAP_BEGIN` / `LOCAL_HEAP_END` が指す領域）とデータスタックの上に作る
- そこで「呼び出しの深さ」ではなく「VM が使うメモリの量」に上限を設ける。再帰が深くなればフレーム・戻り先・データスタックのいずれかが伸びるので、メモリ上限で実質的にコールスタックを制限できる。`__alloc` を繰り返すプログラムや、データスタックに push し続ける手書きの Whitespace も、同じ上限で止まる

## 設計・方針

### 計測するもの

単位はセル（`i64` 1 個）。

```text
使用セル数 = heap.len()（書き込まれたことのあるアドレスの数）
           + data_stack.len()
           + call_stack.len()（戻り先。処理系の内部状態だが、ホストのメモリを消費するため含める）
```

- バイト数ではなくセル数で数える。`HashMap` のオーバーヘッドなどホスト実装に依存せず、決定的で、テストしやすいため。目安として、1 セルはホスト上でおよそ 16〜48 バイトになる（`HashMap` のエントリ、`Vec` の要素）
- 名前は `max_memory_cells`。値の単位（cells）を名前に含める（AGENTS.md の単位規約）。`call` という語は使わない
- データスタックだけの上限や、戻り先だけの上限は別に設けない。合計への上限 1 つで、どの資源が伸びても止まるため。個別の上限が必要になったら、`max_data_stack_cells` のように資源名と単位を名前に入れて、別タスクで追加する

### API

```rust
impl WhitespaceVM {
    /// VM が使うメモリ（ヒープ・データスタック・戻り先の合計セル数）の上限を設定する。None は無制限
    pub fn with_max_memory_cells(self, limit: Option<usize>) -> Self;
}

pub enum WsRuntimeError {
    // 追加
    /// VM が使うメモリが上限を超えた（値は上限のセル数）
    MemoryLimitExceeded(usize),
}
// Display: "memory limit exceeded: {n} cells"
```

- VM の API の既定は `None`（無制限）。既存のテスト、`ws_profiler`、`whitespace20` の既定の挙動は変えない。ただし WASM の `WasmWhitespaceVM::new` だけは、既定を有限にする（[03](03-wasm-whitespace-vm.md)。既存の利用者から見て挙動が変わるので VERSION.md に記載する）
- `Some(0)` は「上限 0 セル」を意味する（VM の API では特別扱いしない）。CLI（`--max-memory-cells 0`）と WASM（`max_memory_cells: 0`）の層で、`0` を `None`（無制限）に変換する
- 推奨値は定数 `whitespace::DEFAULT_MAX_MEMORY_CELLS = 1 << 24`（16,777,216 セル）とする。CLI `nospace20 --mode run`（retire-direct-interpreter）と WASM の `new()` が使う
- ホストのメモリの見積もり: ヒープ中心のプログラムでは `HashMap<i64, i64>` が支配的になる。1600 万エントリを超えるとバケット数が 2^25 になり（約 570MB）、拡張の瞬間は旧テーブル（約 285MB）も同時に確保されるため、ピークは約 0.85GB になる。CLI では許容範囲だが、wasm32 のブラウザ（特にモバイル）ではタブが落ちうる。実装時に CLI とブラウザで実測し、WASM 用の既定値を別の定数（例: `DEFAULT_MAX_MEMORY_CELLS_WASM = 1 << 22`）にすることも検討する。どちらの値も、large テストの最大使用量を十分に上回ること

### 確認する箇所

使用セル数が増えるのは、次の 3 つだけである。増えるときにだけ確認する（増えない操作には確認を入れない）。

| 増える操作 | 確認の位置 |
|---|---|
| ヒープに新しいアドレスを書く | `heap_store`（ws-vm-input-via-heap-store の実施後は、入力命令もここを通る）。確認は、debug 拡張のフック（`-10`〜`-13` への書き込みはヒープに入らない）の分岐の**後**、`heap.insert` の直前に置く。前に置くと、使用量が上限ちょうどのときに `__trace` / `__clog` が誤ってエラーになるため。既存のアドレスへの上書きでは増えないので、上限に達しているときだけ `contains_key` で新規かどうかを確かめる |
| データスタックが正味で増える命令（`Push`、`Duplicate`、`Copy` の 3 つ） | この 3 命令の処理に確認を入れる。`Retrieve`・算術演算・入力待ちでの再 push などは、pop した後に push するので正味は増えず、確認しない（ws-slide-instruction で追加する `Slide` も増えない） |
| `Call` で戻り先を積む | `Call` 命令の処理 |

- 確認は「現在の使用量 + 増分 > 上限」で行う。超えるなら、増やす操作（ヒープへの書き込み、push、戻り先の積み込み）を行わずに `StepResult::Error(MemoryLimitExceeded(limit))` を返す。`Store` や入力命令のように、すでにオペランドを pop している命令では、pop 済みのオペランドは戻さない（既存の `AssertionFailed` と同じ扱い。エラー後に実行を再開することはない）
- 使用量は毎回 3 つの `len()` を足して求める（`O(1)`）。別にカウンタを持つと、ヒープからの削除がないことなど、整合を保つ条件が増えるため、持たない
- 上限が `None` のときは確認の分岐 1 つだけにして、上限なしの実行速度を落とさない（[02](02-clog-debug-ext.md) と同じく、書き込みの高速化を妨げない形にする）
- 確認で使う加算は `checked_add` にする（上限付近の `usize` 計算でもオーバーフローで panic しないように）

### `whitespace20`

- `--max-memory-cells <N>` を追加する。既定は 0（無制限）。汎用の Whitespace 処理系としての既定の挙動を変えないため
- `MemoryLimitExceeded` のエラー表示を追加する（手書きの `match` が残っている場合は、そこに分岐を加える）

## テスト

- WhitespaceVM のユニットテスト:
  - ヒープ: 上限を小さく設定し、新しいアドレスへの書き込みで `MemoryLimitExceeded`。既存のアドレスへの上書きでは増えない
  - データスタック: push の繰り返しで `MemoryLimitExceeded`
  - 戻り先: 自分自身を `Call` し続けるプログラムで `MemoryLimitExceeded`
  - `None` では、従来どおり同じプログラムが上限なしで進む（ステップ数の上限で `Suspended` になる）
- nospace の無限再帰を compile → VM（上限あり）で実行すると、`MemoryLimitExceeded` で止まる。`tests/` の結合テストに置く。再帰は末尾でない形（例: `func: f() { return: f() + 1; }`）にする。自己末尾再帰（`return: f();`）は、予定されている末尾呼び出し最適化でループに変換され、メモリが伸びない無限ループになるため使わない。コンパイラは呼び出しごとに旧 `LOCAL_HEAP_BEGIN` をデータスタックに積み、`Call` で戻り先も積むので、ローカル変数がなくても 1 段あたり約 2 セル伸びる
- `whitespace20` の `--max-memory-cells 0` が `None` に変換されること（引数の変換関数を切り出してユニットテストする）
- `tests_ws/` の manifest（type は `ws_io` のみ）は VM のオプションを渡せないので、large テストは追加しない

## チェックリスト

1. [ ] `WsRuntimeError::MemoryLimitExceeded` と `Display`
2. [ ] データスタックへの push の共通化（必要なら）と、3 か所の確認
3. [ ] `with_max_memory_cells` と `DEFAULT_MAX_MEMORY_CELLS`
4. [ ] `whitespace20 --max-memory-cells`
5. [ ] ユニットテストと結合テスト
