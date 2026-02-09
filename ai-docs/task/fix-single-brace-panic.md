# `}` だけのソースコードで発生するパニックの修正

## 問題の概要

ソースコードに `}` だけを記入して実行しようとすると、プログラムがパニックを起こして異常終了する。

### 現象

```bash
$ echo "}" > test.ns
$ cargo run --bin nospace20 -- test.ns
thread 'main' (9998) panicked at src/interpreter/mod.rs:19:46:
called `Option::unwrap()` on a `None` value
```

### 実行スタック

1. `nospace20::main`
2. `nospace20::interpret_with_env`
3. `nospace20::interpreter::interpret_all`
4. `nospace20::interpreter::interpret_func`
5. `scope.get_function("main").unwrap()` ← ここでパニック

## 根本原因の分析

### 問題の流れ

1. **トークンパーサー段階**
   - `}` は `Token::BraceR` として正常に認識される
   - エラーは発生しない

2. **ツリーパーサー段階**
   - `parse_to_statements` が呼び出される
   - `Token::BraceR` が出現すると、308行目で単に `break` する
   - エラーを記録せず、空の文リストを返す
   - 余ったトークン（`BraceR`）のチェックが行われない

3. **セマンティック解析段階**
   - 空の文リストから空のスコープを生成
   - main 関数の存在チェックが行われない
   - エラーは発生しない

4. **インタプリタ実行段階**
   - `interpret_all` が `interpret_func(env, scope, "main")` を呼び出す
   - `interpret_func` 内で `scope.get_function("main").unwrap()` が実行される
   - main 関数が存在しないため `None` が返され、unwrap() でパニック

### コードの問題箇所

#### 1. ツリーパーサー ([src/tree_parser/statement/mod.rs](../../src/tree_parser/statement/mod.rs#L308))

```rust
Token::BraceR => {
    // TODO: consider only BraceR
    break;
}
```

トップレベルで `BraceR` が出現した場合でもエラーを記録せず、そのまま break している。

#### 2. トップレベルパース後のトークン残存チェック不足

`parse_to_tree` 関数([src/tree_parser/mod.rs](../../src/tree_parser/mod.rs#L22-L31))では、パース後に余ったトークンがないかチェックしていない。

#### 3. main 関数の存在チェック不足

実行モードの場合、main 関数の存在が必須だが、セマンティック解析やインタプリタ起動前にチェックが行われていない。

#### 4. インタプリタの unwrap() ([src/interpreter/mod.rs](../../src/interpreter/mod.rs#L19))

```rust
pub fn interpret_func(env: &mut Environment, scope: &Scope, func_name: &str) -> Option<i64> {
    let func = scope.get_function(func_name).unwrap();
    // ...
}
```

関数が存在しない場合でも unwrap() で即座にパニックしており、適切なエラーメッセージが表示されない。

## 修正方針

複数のレイヤーで問題を防ぐ、多層防御の approach を取る。

### Phase 1: 緊急対応（最小限の修正）

#### 1-1. インタプリタでのエラーハンドリング強化（必須）

**ファイル**: [src/interpreter/mod.rs](../../src/interpreter/mod.rs)

`interpret_func` と `interpret_all` で unwrap() を使わず、適切なエラー処理を行う。

**変更内容**:
- `interpret_func` で関数が存在しない場合、panic の代わりにエラーメッセージを eprintln で出力して None を返す
- または、Result 型を返すように変更

**実装例**:
```rust
pub fn interpret_func(env: &mut Environment, scope: &Scope, func_name: &str) -> Option<i64> {
    let func = match scope.get_function(func_name) {
        Some(f) => f,
        None => {
            eprintln!("error: function '{}' not found", func_name);
            return None;
        }
    };
    let mut e = LocalEnvironment::new_func(env, scope, &func, &Vec::<i64>::new());
    let res = e.interpret_statements(&func.block.statements);
    if let Flow::Return(x) = res {
        Some(x)
    } else {
        None
    }
}
```

#### 1-2. main 関数の事前チェック（推奨）

**ファイル**: [src/bin/nospace20.rs](../../src/bin/nospace20.rs)

実行モードの場合、`interpret_with_env` を呼び出す前に main 関数の存在をチェック。

**変更内容**:
- line 200 付近、`interpret_with_env` 呼び出し前にチェックを追加

**実装例**:
```rust
// モードに応じて処理
match property.mode {
    ExecutionMode::Run => {
        // main 関数の存在チェック
        if a.get_function("main").is_none() {
            eprintln!("error: function 'main' not found");
            process::exit(1);
        }
        
        // インタプリタモード
        let config = nospace20::EnvironmentConfig {
            // ...
        };
        // ...
```

### Phase 2: 構文エラーの検出強化（推奨）

#### 2-1. パース後の余剰トークンチェック

**ファイル**: [src/tree_parser/mod.rs](../../src/tree_parser/mod.rs)

`parse_to_tree` 関数で、パース後にイテレータに残っているトークンがあればエラーとする。

**変更内容**:
```rust
pub fn parse_to_tree(
    tokens: &Vec<PrettyToken>,
) -> Result<Vec<LocatedStatement>, Vec<CodeParseError>> {
    let mut iter = tokens.iter().peekable();
    let (st, mut err) = parse_to_statements(&mut iter);
    
    // 余剰トークンのチェック
    if let Some((_, token_info)) = iter.next() {
        err.push(code_parse_error!(
            token_info.code_pointer,
            "unexpected token (unmatched closing brace or extra code)"
        ));
    }
    
    if err.is_empty() {
        Ok(st)
    } else {
        Err(err)
    }
}
```

#### 2-2. トップレベルの BraceR を明示的にエラーとする（任意）

**ファイル**: [src/tree_parser/statement/mod.rs](../../src/tree_parser/statement/mod.rs)

`parse_to_statements` がトップレベルから呼ばれた場合、`BraceR` でエラーを記録する。

ただし、現状 `parse_to_statements` はブロック内部からも呼ばれるため、トップレベルかどうかを識別する仕組みが必要。Phase 2-1 の修正で十分カバーできるため、これは任意とする。

### Phase 3: コンパイラでの main 関数チェック（将来対応）

コンパイルモードでは main 関数がなくてもライブラリとしてコンパイルできるべきだが、実行可能なバイナリを生成する場合は main 関数が必要。

現状、コンパイルモードでは main 関数の有無に関わらずコンパイルできる仕様なら問題ないが、将来的に実行可能バイナリを生成する際には main 関数チェックが必要になる可能性がある。

## 実装優先度

### 必須
- Phase 1-1: インタプリタでのエラーハンドリング強化
- Phase 1-2: main 関数の事前チェック

### 推奨
- Phase 2-1: パース後の余剰トークンチェック

### 任意
- Phase 2-2: トップレベルの BraceR を明示的にエラーとする

## テストケースの追加

修正後、以下のテストケースを追加すべき:

### 1. `}` だけのコード

**ファイル**: `resources/tests/error/only_closing_brace.ns`

```nospace
}
```

**期待される動作**: 構文エラーまたは「main関数が見つかりません」エラー

### 2. main 関数なしのコード

**ファイル**: `resources/tests/error/no_main_function.ns`

```nospace
func:foo(){
    return:42;
}
```

**期待される動作**: 「main関数が見つかりません」エラー

### 3. 複数の余剰 `}`

**ファイル**: `resources/tests/error/extra_closing_braces.ns`

```nospace
func:main(){
    return:0;
}}
```

**期待される動作**: 構文エラー

## 影響範囲

### 変更が必要なファイル

1. `src/interpreter/mod.rs` - unwrap() の削除、エラーハンドリング追加
2. `src/bin/nospace20.rs` - main 関数の事前チェック追加
3. `src/tree_parser/mod.rs` - 余剰トークンチェック追加
4. `resources/tests/error/` - テストケース追加（新規ディレクトリ）

### 既存機能への影響

- **破壊的変更なし**: エラーケースが正しくエラーとして報告されるようになるだけ
- **既存テストへの影響**: 既存の正常系テストには影響なし
- **パフォーマンス**: 軽微なチェックの追加のみで、パフォーマンスへの影響は無視できる

## 実装手順

1. Phase 1-1 を実装（`src/interpreter/mod.rs`）
2. Phase 1-2 を実装（`src/bin/nospace20.rs`）
3. 動作確認（`}` だけのコードでパニックしないこと）
4. Phase 2-1 を実装（`src/tree_parser/mod.rs`）
5. 動作確認（構文エラーが正しく報告されること）
6. テストケースを追加
7. すべてのテストを実行して回帰がないことを確認
8. コミット

## 備考

- 本修正により、エラーメッセージが改善され、ユーザーフレンドリーになる
- パニックではなく適切なエラーメッセージが表示されることで、デバッグが容易になる
- 多層防御により、同様の問題が発生しにくくなる
