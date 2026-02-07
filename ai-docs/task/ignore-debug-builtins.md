# __assert / __trace 無視オプションの追加

## 概要

コンパイル引数（CLI オプション）に `__assert`、`__assert_not`、`__trace` を無視するオプションを追加する。
無視された場合でも引数は必ず評価される（副作用を保持するため）。

### 背景

- `__assert` / `__assert_not` / `__trace` はデバッグ・テスト用の組み込み関数
- Whitespace コンパイラ (`compiler_ws`) では既に「引数を評価し、最初の引数の値を返す（機能自体は無効）」として実装済み
- インタプリタモードでは常にアサーション・トレースが有効であり、無効化する手段がない
- リリース用ビルドや本番実行時にアサーションを無効化したいケースがある

### 要件

1. `__assert(expr)` — 無視時: `expr` を評価し、その値を返す。パニックしない
2. `__assert_not(expr)` — 無視時: `expr` を評価し、その値を返す。パニックしない
3. `__trace(key)` — 無視時: `key` を評価し、0 を返す。トレース記録しない
4. 引数の副作用は保持される。例: `__assert(a += 2)` では `a += 2` は必ず評価される

## 設計

### CLI オプション

新しい CLI フラグを追加:

```
--ignore-debug    デバッグ用組み込み関数（__assert, __assert_not, __trace, __clog）を無視する
```

`--ignore-debug` は `__assert`, `__assert_not`, `__trace`, `__clog` をすべて一括で無視する。
個別指定（`--ignore-assert` のみ等）は初期実装では行わない。需要が出た場合に拡張する。

### 影響するモジュール

#### 1. `compile_property.rs` — CompileProperty に新フィールド追加

```rust
pub struct CompileProperty {
    pub std: LanguageStd,
    pub mode: ExecutionMode,
    pub target: CompileTarget,
    pub output: Option<String>,
    pub debug: bool,
    pub ignore_debug: bool,  // 追加
}
```

- `ignore_debug: bool` — `true` の場合、デバッグ用組み込み関数を無視する
- デフォルト値は `false`（従来動作を維持）

#### 2. `bin/nospace20.rs` — CLI 引数の追加

```rust
struct Args {
    // ...既存のフィールド...
    
    /// Ignore debug built-in functions (__assert, __assert_not, __trace, __clog)
    #[arg(long)]
    ignore_debug: bool,
}
```

CompileProperty 構築時に `ignore_debug` を渡す。

#### 3. `interpreter/environment.rs` — EnvironmentConfig に設定追加

```rust
pub struct EnvironmentConfig {
    pub max_expression_count: Option<usize>,
    pub ignore_debug: bool,  // 追加
}
```

`EnvironmentConfig` を経由してインタプリタ実行時に参照する。
既存の `new()`, `with_max_expression_count()`, `Default` 実装も更新する（デフォルトは `false`）。

#### 4. `interpreter/exec.rs` — 関数呼び出し時の分岐

`interpret_call_function` メソッド内で `self.env.config.ignore_debug` を参照し、無視時の動作に分岐する。

変更前:
```rust
"__assert" => {
    let a = try_expr!(self.interpret_expression(args.first().unwrap()));
    if a == 0 {
        panic!("assertion failed: {} == 0", a);
    }
    ExpressionFlow::Value(a)
}
```

変更後:
```rust
"__assert" => {
    let a = try_expr!(self.interpret_expression(args.first().unwrap()));
    if !self.env.config.ignore_debug && a == 0 {
        panic!("assertion failed: {} == 0", a);
    }
    ExpressionFlow::Value(a)
}
```

同様に `__assert_not`, `__trace`, `__clog` も分岐を追加する:

- `__assert_not`: `if !self.env.config.ignore_debug && a != 0 { panic!(...) }`
- `__trace`: `if !self.env.config.ignore_debug { /* トレース記録 */ }`
- `__clog`: `if !self.env.config.ignore_debug { println!(...) }`

戻り値は変わらない（`__assert`/`__assert_not` は引数値、`__trace` は 0）。

#### 5. `bin/nospace20.rs` — main() から EnvironmentConfig へ反映

`ExecutionMode::Run` 分岐で `Environment` を生成する際に、`CompileProperty.ignore_debug` を `EnvironmentConfig.ignore_debug` へ渡す。

```rust
ExecutionMode::Run => {
    let config = EnvironmentConfig {
        ignore_debug: property.ignore_debug,
        ..Default::default()
    };
    let mut env = Environment::new_with_config(
        Box::new(std::io::BufReader::new(std::io::stdin())),
        Box::new(std::io::stdout()),
        config,
    );
    let result = interpret_with_env(&mut env, &a);
    // ...
}
```

#### 6. `compiler_ws/expression.rs` — 変更不要

Whitespace コンパイラは既にデバッグ組み込み関数を noop として処理しており、変更不要。

### lib.rs 公開 API への影響

- `interpret_func_testing`, `interpret_func_with_io` — テスト用関数はデバッグ組み込みを有効にしたまま使うため、変更不要
- 新たに `EnvironmentConfig` にフィールドが増えるが、`Default` 実装により後方互換性を維持

### Whitespace インタプリタ (`whitespace/interpreter.rs`) への影響

Whitespace インタプリタは nospace のデバッグ組み込み関数とは独立した拡張命令として `__trace`, `__assert`, `__assert_not` を実装している。
これも同様に `ignore_debug` オプションを参照できるようにすることが望ましいが、初期実装ではスコープ外とする。

## 実装計画

### ステップ 1: CompileProperty 拡張
- `compile_property.rs` に `ignore_debug: bool` フィールドを追加
- `Default` は `false`

### ステップ 2: CLI 引数追加
- `bin/nospace20.rs` の `Args` に `--ignore-debug` フラグを追加
- `CompileProperty` 構築部分で反映

### ステップ 3: EnvironmentConfig 拡張
- `interpreter/environment.rs` の `EnvironmentConfig` に `ignore_debug: bool` を追加
- `new()`, `with_max_expression_count()`, `Default` 実装を更新

### ステップ 4: インタプリタの分岐実装
- `interpreter/exec.rs` の `interpret_call_function` を修正
- `__assert`, `__assert_not`, `__trace`, `__clog` の各分岐に `ignore_debug` チェックを追加

### ステップ 5: main() の接続
- `bin/nospace20.rs` の Run モードで `EnvironmentConfig` に `ignore_debug` を伝搬

### ステップ 6: テスト
- Unit テスト: `interpreter/exec.rs` に `ignore_debug=true` のテストを追加
  - `__assert(0)` でパニックしないことを確認
  - `__assert_not(1)` でパニックしないことを確認
  - `__assert(a += 2)` で `a` が 2 になることを確認（副作用保持）
  - `__trace(1)` でトレースが記録されないことを確認
- Large テスト: `resources/tests/` に `--ignore-debug` を使うテストケースの追加を検討

## 規模見積もり

- 変更ファイル数: 4〜5 ファイル
- 追加コード量: 約 30〜50 行
- 既存テストへの影響: なし（デフォルト `false` で従来動作を維持）
- 難易度: 低〜中
