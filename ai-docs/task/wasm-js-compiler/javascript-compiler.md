# JavaScript コンパイラ詳細設計

## 概要

nospace の `Scope`（意味解析結果）を入力として、JavaScript ソースコードを文字列として出力する。
出力された JS コードは、ランタイムオブジェクトを注入することで Node.js やブラウザで実行可能。

## 出力形式

### 全体構造

```javascript
// nospace compiled output
"use strict";

// === Runtime Interface ===
// ランタイムオブジェクトは外部から注入される
// デフォルトではコンソールベースの簡易実装を組み込み
const __nospace_runtime = (typeof __nospace_runtime_inject !== "undefined")
  ? __nospace_runtime_inject
  : {
      putInt: (x) => { process.stdout.write(String(x)); },
      putChar: (x) => { process.stdout.write(String.fromCharCode(x)); },
      getInt: () => { /* デフォルト実装は 0 を返す */ return 0; },
      getChar: () => { /* デフォルト実装は 0 を返す */ return 0; },
    };

// === Global Variables ===
let __g_0 = 0;  // グローバル変数 (例: global_var)
let __g_1 = 0;  // static 変数 (例: static_var)

// === Global Initialization ===
// root_statements に対応
__g_0 = 42;

// === Function Definitions ===
function add(a, b) {
  return (a + b);
}

function main() {
  let __v_0 = 0; // ローカル変数
  __v_0 = add(3, 4);
  __nospace_runtime.putInt(__v_0);
  return 0;
}

// === Entry Point ===
main();
```

### ランタイム注入モデル

出力 JS コードは `__nospace_runtime_inject` というグローバル変数が存在すればそれを使用し、
なければデフォルトの Node.js 向け実装にフォールバックする。

別リポジトリ側では以下のように注入する:

```javascript
// ブラウザ側の例
window.__nospace_runtime_inject = {
  putInt: (x) => { outputElement.textContent += String(x); },
  putChar: (x) => { outputElement.textContent += String.fromCharCode(x); },
  getInt: () => { return parseInt(inputQueue.shift() || "0"); },
  getChar: () => { return (inputQueue.shift() || "\0").charCodeAt(0); },
};

// nospace コンパイル結果をロード
import "./compiled.js";
```

## 変換規則

### 変数

| nospace | JavaScript | 備考 |
|---------|-----------|------|
| ローカル変数 | `let __v_{index} = 0;` | インデックスベースの命名 |
| 関数引数 | そのまま引数名を使用 | `function f(a, b)` |
| グローバル変数 | `let __g_{index} = 0;` | トップレベルに配置 |
| static 変数 | `let __g_{index} = 0;` | グローバル変数と同じ領域に配置 |

変数名はインデックスベースにする（`__v_0`, `__v_1`, ...）。
これにより nospace の識別子と JS の予約語の衝突を回避する。

ただし、デバッグ用途として元の変数名をコメントに記載するオプションを提供可能。

### 式

| nospace | JavaScript | 備考 |
|---------|-----------|------|
| `a + b` | `(a + b)` | 括弧で囲み優先順位を明示 |
| `a - b` | `(a - b)` | |
| `a * b` | `(a * b)` | |
| `a / b` | `Math.trunc(a / b)` | 整数除算にするため `Math.trunc` |
| `a % b` | `(a % b)` | JS の `%` は剰余（nospace と同じ） |
| `-a` | `(-a)` | 単項マイナス |
| `!a` | `(a === 0 ? 1 : 0)` | nospace の `!` は 0→1, 非0→0 |
| `a == b` | `(a === b ? 1 : 0)` | 結果は 0 or 1 |
| `a != b` | `(a !== b ? 1 : 0)` | |
| `a < b` | `(a < b ? 1 : 0)` | |
| `a <= b` | `(a <= b ? 1 : 0)` | |
| `a > b` | `(a > b ? 1 : 0)` | |
| `a >= b` | `(a >= b ? 1 : 0)` | |
| `a && b` | `(a !== 0 && b !== 0 ? 1 : 0)` | 短絡評価を保持 |
| `a \|\| b` | `(a !== 0 ? a : b)` | 短絡評価、左辺が非0ならその値を返す |
| `a = b` | `a = (b)` | 代入は式として値を返す |

### 短絡評価の詳細

`&&` と `||` は短絡評価を行う。右辺に副作用がある場合、評価順序を保持する必要がある。

```javascript
// nospace: a && b (a, b は式)
// a が 0 なら b を評価せず 0 を返す
(((__t = (a_expr)) !== 0) ? ((b_expr) !== 0 ? 1 : 0) : 0)

// nospace: a || b
// a が非0なら b を評価せず a の値を返す
(((__t = (a_expr)) !== 0) ? __t : (b_expr))
```

副作用のある式（関数呼び出しなど）が含まれている場合、一時変数を使用する。
単純な変数参照・リテラルの場合は一時変数なしで最適化可能。

### 文

| nospace | JavaScript |
|---------|-----------|
| `return: expr;` | `return (expr);` |
| `break;` | `break;` |
| `continue;` | `continue;` |
| 式文 | `expr;` |

### 制御構文

#### if 文

```javascript
// nospace: if: cond { ... } else: { ... };
// if は式として扱われるが、現在は常に 0 を返す
if ((cond) !== 0) {
  // then block
} else {
  // else block
}
```

#### while 文

```javascript
// nospace: while: cond { ... };
while ((cond) !== 0) {
  // body
}
```

### 関数定義

```javascript
// nospace: func: add(a, b) { return: a + b; }
function add(a, b) {
  return (a + b);
}
```

- ホイスティング: JS の `function` 宣言も自動的にホイストされるため、nospace と同じ動作
- 戻り値なし: `return` がない場合は末尾に `return 0;` を追加

#### ネストした関数

nospace は関数内に関数を定義できる。JS でも同様の構造にする:

```javascript
function outer() {
  function inner() {
    // ...
  }
  inner();
}
```

### 組み込み関数

| nospace | JavaScript |
|---------|-----------|
| `__puti(x)` | `__nospace_runtime.putInt(x)` ※戻り値は x |
| `__putc(x)` | `__nospace_runtime.putChar(x)` ※戻り値は x |
| `__geti()` | `__nospace_runtime.getInt()` |
| `__getc()` | `__nospace_runtime.getChar()` |
| `__clog(x)` | `console.log(x)` ※戻り値は x |
| `__trace(key)` | `/* noop */0` |
| `__assert(x)` | `((x) === 0 ? (() => { throw new Error("assert"); })() : (x))` |
| `__assert_not(x)` | `((x) !== 0 ? (() => { throw new Error("assert_not"); })() : (x))` |

**戻り値の扱い**: `__puti(x)` は `x` を返す仕様のため、式として使われる場合は
`((v) => { __nospace_runtime.putInt(v); return v; })(expr)` のようなラッパーが必要。
ただし、文として使われる場合は単純に `__nospace_runtime.putInt(expr);` でよい。

## スコープの走査順序

`Scope` 構造を以下の順序で走査し、JavaScript コードを生成する:

1. **ランタイム定義**: `__nospace_runtime` の注入コード
2. **グローバル変数宣言**: `Scope.variables` をイテレートし、`let __g_{i} = 0;` を生成
3. **グローバル初期化コード**: `Scope.root_statements` を走査し、初期化文を生成
4. **関数定義**: `Scope.functions` を再帰的に走査し、`function` 宣言を生成
   - 各関数内で:
     a. ローカル変数宣言
     b. 文の生成
5. **エントリポイント**: `main();` を出力

## モジュール設計

### mod.rs

```rust
pub fn compile(scope: &Scope) -> Result<String, CompileError> {
    let mut ctx = JsCodeGenContext::new(scope);
    let mut output = String::new();
    
    // 1. ヘッダー（ランタイム定義）
    output.push_str(&generate_runtime_header());
    
    // 2. グローバル変数
    output.push_str(&generate_global_variables(&ctx, scope)?);
    
    // 3. グローバル初期化
    output.push_str(&generate_global_init(&mut ctx, scope)?);
    
    // 4. 関数定義
    output.push_str(&generate_functions(&mut ctx, scope)?);
    
    // 5. エントリポイント
    output.push_str("main();\n");
    
    Ok(output)
}
```

### context.rs

```rust
pub struct JsCodeGenContext<'a> {
    scope: &'a Scope,
    indent_level: usize,
    temp_var_counter: usize,  // 一時変数のカウンタ
}
```

### expression.rs

式を JS 文字列に変換する関数群。
`ExecExpression` をパターンマッチして再帰的に JS コードを生成する。

### statement.rs

文を JS 文字列に変換する関数群。
`ExecStatement` をパターンマッチして再帰的に JS コードを生成する。

### runtime.rs

ランタイムヘッダーテンプレート、組み込み関数のブリッジコードを生成する。

## 変数のアドレッシング

### IdentifierRef の解決

nospace の `IdentifierRef` は `(scope_depth, local_index, is_global)` で変数を参照する。

JS コード生成では:

- `is_global == true` → `__g_{local_index}` を参照
- `is_global == false` → 現在のスコープ深度に応じた変数名を参照

#### ローカル変数のスコープ対応

nospace のブロックスコープは JS のブロックスコープに直接マッピングする:

```javascript
function example() {
  let __v_0 = 0;  // 関数スコープの変数
  if (1) {
    let __v_1 = 0;  // ブロックスコープの変数
    __v_0 = 1;      // 親スコープの変数にアクセス
  }
  // __v_1 はここではアクセス不可（JS の let のスコープルール）
}
```

`scope_depth` が 0 でない場合、親スコープの変数を参照するが、
JS の `let` はブロックスコープなので、親ブロックの変数名をそのまま使えば解決される。

ただし、関数スコープ境界を越える場合（static 変数のみ）は `__g_` プレフィックスの変数を使う。

## テスト方針

1. 単体テスト: 各変換規則の正しさを検証
2. 統合テスト: nospace ソース → JS コンパイル → Node.js 実行 → 出力比較
3. 既存テストケース: `resources/tests/` のテストケースを流用

統合テストでは `node` コマンドを外部プロセスとして実行する。
