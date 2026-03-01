# Phase 2+3 テスト失敗調査

## 背景

Phase 2+3（Environment アロケータ移行、LocalEnvironment スコープ管理移行）の実装後に
`src/interpreter/exec.rs` の既存テスト 2 件が失敗している。

## 失敗テスト一覧

| テスト名 | ファイル | 失敗原因 |
|---|---|---|
| `test_resolve_address_local_variables` | `src/interpreter/exec.rs:732` | アドレスが 0 ではなく 1 になる |
| `test_get_set_by_address` | `src/interpreter/exec.rs:768` | アドレス 0 が未割当アドレスになる |

## 失敗の詳細

### `test_resolve_address_local_variables`

```
assertion `left == right` failed: x should be at address 0
  left: 1
 right: 0
```

**原因**: 旧アドレスモデルでは `resolve_address` がスコープスタックの配列インデックスを直接返していた
（ローカル変数 x は `scope_stack[0][0]` → アドレス 0）。

新アドレスモデル（Phase 2+3）では `alloc_internal_uninit` がアロケータの仮想アドレスを返す。
アロケータは `next_addr = 1` から割り当てを始めるため（0 はフリーリストのセンチネル値）、
ローカル変数 x のアドレスは `base_addr + 0 = 1`（具体的な値はアロケータの状態による）。

**修正方針**: このテストは「ローカル変数がアドレス 0 から始まる」という旧仮定をアサートしている。
新モデルでは「`resolve_address` が返すアドレスを使って `get_variable` / `set_variable` で正しくアクセスできる」
という観点でテストを書き直す必要がある。

```rust
// 修正例
let addr_x = local_env.resolve_address(&id_x);
// アドレスの絶対値ではなく、アドレスが利用可能であることを確認
local_env.env.allocator.set(addr_x, 42);
assert_eq!(local_env.env.allocator.get(addr_x), 42);
```

### `test_get_set_by_address`

```
thread panicked at src/interpreter/allocator.rs:153:32:
runtime error: invalid memory access at address 0
```

**原因**: テストが `local_env.set_by_address(0, 42)` を呼び出している。
旧アドレスモデルでは 0 がローカル変数 x のアドレスだったが、
新アドレスモデルではアドレス 0 はフリーリストの空を表すセンチネル値として予約されており、
未割当アドレスへのアクセスは panic する。

**修正方針**: このテストは「アドレス 0 と 1 を直接設定できる」という旧 API の仮定をテストしている。
新モデルでは `new_func` で確保されたスコープのアドレスを使って `set/get_by_address` をテストする
必要がある。

```rust
// 修正例
let addr_x = local_env.resolve_address(&id_x);
let addr_p = local_env.resolve_address(&id_p);
local_env.set_by_address(addr_x, 42);
assert_eq!(local_env.get_by_address(addr_x), 42);
local_env.set_by_address(addr_p, 99);
assert_eq!(local_env.get_by_address(addr_p), 99);
```

## 対処状況

Phase 5 で修正済み。両テストとも `resolve_address` が返す実際のアロケータアドレスを使う形に書き直した。

## 関連コミット

- Phase 2+3 実装コミット（この変更と同じコミット）
