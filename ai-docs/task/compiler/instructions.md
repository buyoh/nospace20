# Whitespace 命令セット

旧実装 `main.cpp` で使用されている Whitespace 命令の一覧。

## 命令エンコーディング

Whitespace の命令は、IMP（Instruction Modification Parameter）とコマンドの組み合わせで構成されます。

- `SP` = スペース (0x20)
- `TB` = タブ (0x09)
- `LF` = 改行 (0x0A)

## スタック操作 (IMP: SP)

| 命令 | エンコーディング | 説明 |
|------|------------------|------|
| push | `SP SP <number>` | 数値をスタックにプッシュ |
| duplicate | `SP LF SP` | スタックトップを複製 |
| copy | `SP TB SP <n>` | n番目の要素をコピーしてプッシュ |
| swap | `SP LF TB` | スタックトップの2要素を交換 |
| discard | `SP LF LF` | スタックトップを破棄 |

## 算術演算 (IMP: TB SP)

| 命令 | エンコーディング | 説明 |
|------|------------------|------|
| add | `TB SP SP SP` | 加算: push(pop() + pop()) |
| sub | `TB SP SP TB` | 減算: a=pop(), b=pop(), push(b-a) |
| mul | `TB SP SP LF` | 乗算: push(pop() * pop()) |
| div | `TB SP TB SP` | 除算: a=pop(), b=pop(), push(b/a) |
| mod | `TB SP TB TB` | 剰余: a=pop(), b=pop(), push(b%a) |

## ヒープアクセス (IMP: TB TB)

| 命令 | エンコーディング | 説明 |
|------|------------------|------|
| store | `TB TB SP` | addr=pop(), val=pop(), heap[addr]=val |
| retrieve | `TB TB TB` | addr=pop(), push(heap[addr]) |

## フロー制御 (IMP: LF)

| 命令 | エンコーディング | 説明 |
|------|------------------|------|
| label | `LF SP SP <label>` | ラベル定義 |
| call | `LF SP TB <label>` | サブルーチン呼び出し |
| jump | `LF SP LF <label>` | 無条件ジャンプ |
| zerojump | `LF TB SP <label>` | 0ならジャンプ |
| negativejump | `LF TB TB <label>` | 負数ならジャンプ |
| return | `LF TB LF` | サブルーチンから戻る |
| exit | `LF LF LF` | プログラム終了 |

## I/O (IMP: TB LF)

| 命令 | エンコーディング | 説明 |
|------|------------------|------|
| putchar | `TB LF SP SP` | スタックトップを文字として出力 |
| putnumber | `TB LF SP TB` | スタックトップを数値として出力 |
| getchar | `TB LF TB SP` | 文字を読み込みヒープに格納 |
| getnumber | `TB LF TB TB` | 数値を読み込みヒープに格納 |

## 数値エンコーディング

数値は以下の形式でエンコードされます：

```
<符号> <2進数ビット列> LF
```

- 符号: `SP` = 正, `TB` = 負
- ビット列: `SP` = 0, `TB` = 1（MSB first）

### 例

```
整数 5 (2進数: 101):
  SP     (正)
  TB     (1)
  SP     (0)
  TB     (1)
  LF     (終端)

整数 0:
  SP     (正)
  LF     (終端、ビットなし)

整数 -3 (2進数: 11):
  TB     (負)
  TB     (1)
  TB     (1)
  LF     (終端)
```

## 実装コード参照

```cpp
namespace WS::Instruments {
    namespace Stack {
        const Chr push[] = { Chr::SP, Chr::SP };
        const Chr duplicate[] = { Chr::SP, Chr::LF, Chr::SP };
        const Chr copy[] = { Chr::SP, Chr::TB, Chr::SP };
        const Chr swap[] = { Chr::SP, Chr::LF, Chr::TB };
        const Chr discard[] = { Chr::SP, Chr::LF, Chr::LF };
    }
    namespace Arithmetic {
        const Chr add[] = { Chr::TB, Chr::SP, Chr::SP, Chr::SP };
        const Chr sub[] = { Chr::TB, Chr::SP, Chr::SP, Chr::TB };
        const Chr mul[] = { Chr::TB, Chr::SP, Chr::SP, Chr::LF };
        const Chr div[] = { Chr::TB, Chr::SP, Chr::TB, Chr::SP };
        const Chr mod[] = { Chr::TB, Chr::SP, Chr::TB, Chr::TB };
    }
    namespace Heap {
        const Chr store[] = { Chr::TB, Chr::TB, Chr::SP };
        const Chr retrieve[] = { Chr::TB, Chr::TB, Chr::TB };
    }
    namespace Flow {
        const Chr label[] = { Chr::LF, Chr::SP, Chr::SP };
        const Chr call[] = { Chr::LF, Chr::SP, Chr::TB };
        const Chr jump[] = { Chr::LF, Chr::SP, Chr::LF };
        const Chr zerojump[] = { Chr::LF, Chr::TB, Chr::SP };
        const Chr negativejump[] = { Chr::LF, Chr::TB, Chr::TB };
        const Chr retun[] = { Chr::LF, Chr::TB, Chr::LF };
        const Chr exit[] = { Chr::LF, Chr::LF, Chr::LF };
    }
    namespace IO {
        const Chr putchar[] = { Chr::TB, Chr::LF, Chr::SP, Chr::SP };
        const Chr putnumber[] = { Chr::TB, Chr::LF, Chr::SP, Chr::TB };
        const Chr getchar[] = { Chr::TB, Chr::LF, Chr::TB, Chr::SP };
        const Chr getnumber[] = { Chr::TB, Chr::LF, Chr::TB, Chr::TB };
    }
}
```
