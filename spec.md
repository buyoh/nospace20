# spec

これらの仕様は、今後必ず変わります

## 1 四則演算

```
2+3;
2 + 3;
2 - 3*5;
(2-3)*5;
3*4/(1+2);
```

## 2 組み込み識別子

```
__clog(5);
```

## 3 代入・変数定義

```
let:x; let:y; x=3; y=2; x=x+y; y=x+y;
```

## 4 関数定義

```
func: pow(a) {
  __clog(a);
  return: a*a;
}

func: main() {
  let: x1; let: y1;
  let: x2; let: y2;
  x1 = 3; y1 = 4;
  x2 = 7; y2 = 3;
  __clog(pow(x2 - x1) + pow(y2 - y1));
  return: 0;
}
```
