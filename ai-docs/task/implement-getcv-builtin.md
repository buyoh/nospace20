# TODO: __getcv 組み込み関数の実装

## 概要

`test_legacy_018` が失敗している。`__getcv(&c)` という組み込み関数が実装されていないため。

## 問題の詳細

### エラー内容

```
thread 'test_legacy_018' (50624) panicked at src/interpreter/exec.rs:211:62:
called `Option::unwrap()` on a `None` value
```

### 原因

- `src/interpreter/exec.rs:211` の `self.root_scope.get_function(id.as_str()).unwrap()` が `None` を返している
- `__getcv` という関数が登録されていない

### 仕様

spec.md の記載:

```
| `__getcv(p)` | 標準入力から1文字を読み込み、ASCII値を参照 p へ格納 |
```

### テストケース

`resources/tests/passes/legacy/legacy_018.ns`:

```nospace
func:main(){
    let:c;
    __puti(__getc() == 'A');
    __puti(__getc() == 'a');
    __puti(__getc() == '+');
    __puti(__getc() == '\'');
    __puti(__getc() == '\s');
    __puti(__getc() == '\\');
    __puti(__getc() == '\t');
    __puti(__getc() == '.');
    __puti(__getc() == 0); # CR = 13 #
    __puti(__getc() == '\n');
    __puti(__getcv(&c) == 'X');
    __puti(c == 'X');
}
```

## 実装に必要な作業

1. インタプリタ側で `__getcv` を組み込み関数として実装
   - 引数は変数のアドレス (参照) を受け取る
   - 標準入力から1文字を読み込み、そのアドレスに書き込む
   - 戻り値として読み込んだ値を返す

2. コンパイラ (Whitespace) 側でも同様に実装が必要

## 関連情報

- `ai-docs/done-task/io-builtin-implementation.md` に以下の記載あり:
  > `__getiv`、`__getcv`（アドレス指定入力）は spec.md に記載がないため、今回のスコープ外とする。

- しかし、実際には spec.md に記載がある

- `.local/nospace/main.cpp` (旧実装) には実装がある:
  ```cpp
  reservedNameTable.defineEmbeddedFunction("__getcv", Embedded::Function::IDgetcv, 1);
  ```

## 優先度

中 - legacy テストの一部が失敗するが、基本的な機能は動作している
