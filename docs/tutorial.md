# Nospace Tutorial

Original: https://github.com/buyoh/nospace/blob/master/docs/docs/tutorial.md

## Hello World

```
func: __main() {
    __putc('H');
    __putc('i');
    __putc('\s');
    __putc('H');
    __putc('e');
    __putc('l');
    __putc('l');
    __putc('o');
    _ _ p u t c('\n');
}
```

- `func` で関数を定義出来る
- `__main` 関数を定義しなければならない
- `__main` 関数が呼ばれて実行される
- `__putc()` は文字を出力する
- `_` 2つから始まる関数は組み込み関数
- `'a'` はasciiコード
  - `\` はエスケープ文字
  - バックスラッシュは `\\`
  - 改行(LF)は `\n`
  - 半角スペースは `\s`。
  - タブは `\t`
  - `'` は `\'`
- 空白改行タブを一切含めること無く記述できるのが本言語の下らない特徴
- 逆に、どこに空白改行タブを入れてもその空白は無視される
  - `' '` は `''`と解釈され、コンパイルエラーになる

## Fibonacci

```
# calculate fibonacci(n) #
func: fibo(n) {
    if: n < 0 {
        return: 0;
    } else: if: n == 0 || n == 1 {
        return: 1;
    } else: {
        return: fibo(n-1) + fibo(n-2);
    };
}
func: __main() {
    let: n;
    n = __geti();
    __puti(fibo(n));
    __putc('\n');
}
```

- `return` で関数の返り値を指定出来る
- コメントは `#` で囲む
- 変数宣言は `let`
- 同一スコープで同じ名前の変数・関数の宣言は出来ない
- `__geti()` で数字を読み込む
  - whitespace interpreter側の実装依存。一般的には改行区切りで読み込まれる。

## Swap

```
func: swap(p, q) {
    let: t;
    t = *p;
    *p = *q;
    *q = t;
}
func: __main() {
    let: a(1), b(2);
    swap(&a, &b);
    __puti(a);
    __puti(b);
}
```

- `&a` は変数 `a` の参照を取得
- `*p` は `p` をデリファレンス（間接参照）する
- `let` 宣言の時 `a(1)` で変数 `a` を `1` に初期化する

## Rotate Array

```
f u n c:swap(p,q){let:t;t=*p;*p=*q;*q=t;}
func: rotate(begin, end) {
    end -= 1;
    while: begin < end {
        swap(end - 1, end);
        end -= 1;
    };
}
func: __main(){
    let: arr[]([__getc(), __getc(), __getc(), __getc()]);
    rotate(&arr, &arr+4);
    __putc(arr[0]);
    __putc(arr[1]);
    __putc(arr[2]);
    __putc(arr[3]);
}
```

- `while`
- 配列の宣言は `let:arr[4];`
  - 配列サイズは定数のみ指定可能
  - 初期値を設定した場合は省略できる
- 配列も`let` 宣言の時に初期化出来る
- `a[3]([1,2])` で `a[0]` を `1`，`a[1]`を `2` に初期化する
  - `a[2]` は確保されるが未定値
- `a[]([1,2])` でサイズを省略できる（`a[2]` と同等）
- `arr[1]` で 1番目の要素を参照する
- `arr` は `arr[0]` と同義。C言語と異なる

## Linked stack

```
let: tail(0);

func: push_back(val) {
  let: next(__alloc(2));
  (*next)[0] = tail;
  (*next)[1] = val;
  # *(next + 0) = tail; #
  # *(next + 1) = val; #
  tail = next;
}

func: pop_back() {
  if: tail == 0 {
    return: 0; 
  };
  let: val((*tail)[1]);
  let: p(tail);
  tail = (*tail)[0];
  __free(p);
  return: val;
}

func: __main() {
  for: {let: c(0); } {
    c = __getc();
    c > 32 && c != '$';
  } {} {
    if: '0' <= c && c <= '9' {
      push_back(c - '0');
    };
    if: c == 'p' {
      __puti(pop_back()); 
    };
  };
}
```

- `__alloc(size)` でメモリを確保、`__free(ptr)` で解放する
- `for` 文は 4 つのブロックから構成される: 初期化ブロック, 条件ブロック, 更新ブロック, 本体ブロック

## Sorting (Quick Sort)

```
func: swap(p, q) {
    let: t;
    t = *p; *p = *q; *q = t;
}
func: qsort(begin, end), alias: func: compare(l,r) {
    if: end - begin <= 1 { return:; };
    let: pv(begin), it(begin + 1);
    while: it < end {
        if: !compare(*pv, *it) {
            swap(pv + 1, it);
            swap(pv, pv + 1);
            pv += 1;
        };
        it += 1;
    };
    qsort(begin, pv);
    qsort(pv+1, end);
}

func: lesser(l, r) {
  return: l < r;
}

func: greater(l, r) {
  return: l > r;
}

alias: puti(__puti);
alias: putc(__putc);

alias: qsort_le(qsort, lesser);
alias: qsort_ge(qsort, greater);

func: __main() {
    let: arr[]([3,1,4,1,5,9,2,6,5]);
    qsort_le(&arr, &arr+9);
    repeat: i(0), 9, {
        puti(arr[i]);
        putc('\s');
    };
    let: arr2[]([3,1,4,1,5,9,2,6,5]);
    qsort_ge(&arr2, &arr2+9);
    repeat: i(0), 9, {
        puti(arr2[i]);
        putc('\s');
    };
}
```

- `alias` で関数や変数などの識別子の別名を定義できる。
- `alias` はテンプレート関数の引数としても使用する。
  - 関数定義に `alias` パラメータを追加したとき、その関数は実体を持たないテンプレート関数となる。
  - テンプレート関数を `alias` 定義で参照し、パラメータに対応する識別子を渡すことで、そのテンプレート関数のインスタンス化が行われる。
