# done

## ホイスティング＋初期化

```
x = 5;
let: x(9);
__clog(x);  # 5 を出力するのか？9 を出力するのか？ #
```

```
# x = undefined #
x = 5;
# x = 5 #
let: x(9);
# x = 9 #
```


## static

- 変数の初期化タイミングは global 変数と同様、main が呼び出される前。変数なと定数以外で初期化できない
- static 変数は、static でない global変数より先に初期化される

## 型 void のみ

型システムは導入しないが、内部的に `int` と `void` 型を導入する。（明示的な型定義を実装しない）

- `while` の返り値は `void`
- `if`, `else` チェインについて、1つでも `void` を返すブロックがあれば、全体の返り値は `void` になる。`else`が無い場合は`void` とみなす。
- ユーザ定義関数について、返り値がない場合は `void` とみなす。返り値がある場合は、返り値の型を `int` とみなす。混在する場合はエラー。

この実装によって一部のテストに影響がでる可能性があるため、テストの修正も必要

## B 予約語（未実装）

未定義
予約すべき構文は全て`:`が末尾に来るため、予約語の衝突は起こりにくい。
それでも予約語が必要かどうか？

```
func: func(return) {
  let: let;
  let = return - 1;
  let: if(let > 0);
  if: if {
    return: func(let) + 1;
  } else: {
    return: 0;
  };
}
```

## F ヒープ

### whitespace

heapは1つだけ。全面をstackに使っている。
heap用のheap（？）を用意するには、1つのheapにstackとheapを共存させる必要がある。

- 負のアドレスを使う
  - 一部のwhitespaceインタプリタは、負のアドレスをサポートしていない可能性がある。
- 偶数アドレスをstack、奇数アドレスをheapにする
  - コード上では、隣接した配列の要素のアドレスの差は1であって欲しい。変換する場合、`real_address = logical_address * 2 + (stack_or_heap ? 1 : 0)` のような変換が必要になるが、`stack_or_heap` をどのように管理するかが問題になる。
- メモリプールを実装する
  - 断片化の問題がある。実装も複雑になる。


## G 便利な構文・糖衣構文

### G-1 repeat

while の糖衣構文と定義したいが、`continue` が呼び出されたときもインクリメントしたいため、`i = i + 1;` の配置位置が存在しない。

```
repeat: i(0), 5 {
  __clog(i);  # 0 1 2 3 4 を出力 #
};
```

そこで、 for を定義して、repeatは for の糖衣構文とする（解釈できるようにする）
省略は出来ないが、`{}`と書けば空になる

```
# 変数宣言、初期化、条件式、更新式、ループブロック #
for: i(0), i=0, i<5, i+=1 { __clog(i); };
```

構文変えたくなってきた。ブロックの前に`,`を入れたい。

変数宣言はletと同様の特殊な構文だが、以降は全て式なので、馴染み深い書き方は、

```
for: i(0), {}, i<5, i+=1, {
  __clog(i);
};
```

ただ、ifでrejectしたときと同様に、for, repeat をvoidを返す式として扱うと、関数呼び出しの中に含めることが可能になり、構文の区切り`,`が曖昧になる。
なので、式にすることを諦め、`return` `let` 等と同様に文として扱う。
`while` も式にする理由が無いので、文として扱うことにする。
スコープ式を使うことで、文を含めることができるため、表現力は十分である。

```
__puti__({let: x(0); repeat: i(0), 5, x += i; x;});
```

for はブロックの方が良いかも

```
# 初期化ブロック、条件ブロック、更新ブロック、ループブロック #
for: { let: i(0); }, { i<5; }, { i+=1; }, { __clog(i); };
```



## J テンプレート関数のようなもの

関数ポインタの代替。引数に alias を指定できる。
alias の中身が分からないため、構文相当の情報の伝達が必要

```
# ダメな設計例: compare_func の情報が不足 #
func: sort_by(arr), alias: compare_func {
  # arr を compare_func を使ってソートする #
}
# 案 `func:` で構文を伝える。 #
func: sort_by(arr), alias: func: compare_func(a,b) {
  # arr を compare_func を使ってソートする #
}
func: find_of(arr), alias: constexpr: low, alias: constexpr: high {
  # arr の中から low 以上 high 以下の要素を探す #
}
func: counter(), alias: static: inc {
  static: count(0);
  count = count + inc;
  return: count;
}
```

そのままでは呼び出せず、バイナリも生成しない。alias を使って、alias 引数を具体的な関数や値に置き換えると、関数が生成される。スコープも生成されるため、関数内にある static も alias ごとに独立して存在する

```
alias: sort_by_impl(sort_by, compare_string);
alias: find_of_impl(find_of, 10, 99);
alias: counter_inc1(counter, 1);
alias: counter_inc10(counter, 10); # counter_inc1 と counter_inc10 は独立してカウントする #

sort_by_impl(my_array);
find_of_impl(my_array);
counter_inc1(); # 1 を出力 #
counter_inc1(); # 2 を出力 #
counter_inc10(); # 10 を出力 #
counter_inc10(); # 20 を出力 #
```


## 最適化・高速化

### 設計

- 意味解析の後に、意味解析最適化フェーズを追加する。
- 最適化の項目ごとに、プラグイン形式のような形で設計できるとよい。必要に応じて各最適化を有効化・無効化できるようにする。

### テスト・評価

これを評価する環境を作るのが先。

- 生成命令数（whitespaceの長さではない）
- 実行ステップ数

### 意味解析最適化

- 使っていない関数・変数の削除

### whitespace コンパイル最適化

しかし以下に列挙するものはいずれも「意味解析最適化」で実装する内容。
意味解析最適化以降でのみ使用される関数や構文を定義する。
ドキュメントも必要。

#### `__geti` / `__getiv` / `__getcv` の最適化

- `p = __geti()` はwhitespaceにとって余計な命令列を生成する。
- 過去には `__getiv(&p)`, `__getcv(&p)` が存在していた。
- 意味解析最適化以降でのみ使用される関数として、`__internal_getiv(&p)`, `__internal_getcv(&p)` を定義する。

#### `if`, `while` 文の最適化

- 条件付きgotoは、スタックの最上位がゼロの場合、 スタックの最上位が負の場合。
- nospaceの言語仕様のif文は、条件式がゼロでない場合に真とみなす。
- 次のような最適化が考えられる。`while` 文も同様。
  - 条件式が定数の場合、条件式を評価して、ブロックスコープに変換
  - 条件式が`== 0`の場合、特殊構文 `__if_zero` に変換。
  - 条件式が`< 0`の場合、特殊構文 `__if_negative` に変換。
  - 条件式が`>= 0`の場合、特殊構文 `__if_negative` を使うよう構造を変換。
