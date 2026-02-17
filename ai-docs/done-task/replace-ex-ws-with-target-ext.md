# --target ex-ws を廃止し --target-ext を追加

## 概要

`--target ex-ws` を廃止し、代わりに `--target-ext` オプションを追加する。
`--target-ext` は拡張命令などを有効化するオプションで、複数同時指定が可能。

## 背景

- 現在 `--target ex-ws` は未実装のまま残っている
- 拡張機能をターゲットではなく、オプションとして指定できるようにする
- 複数の拡張を同時に有効化できるようにする

## 要求仕様

### 削除するもの

- `--target ex-ws` オプション
- `CompileTarget::ExWs` 列挙型

### 追加するもの

- `--target-ext <EXT>` オプション（複数指定可能）
- 現在は `debug` のみを用意
- 今後、他の拡張も追加される予定

### 動作

```bash
# debug拡張を有効にしてコンパイル
nospace20 --mode=compile --target=ws --target-ext debug source.ns

# 複数の拡張を指定（将来）
nospace20 --mode=compile --target=ws --target-ext debug --target-ext other source.ns
```

## 設計

### TargetExtension 列挙型

```rust
/// ターゲット拡張
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetExtension {
    /// デバッグ拡張
    Debug,
}
```

### CompileProperty への追加

```rust
pub struct CompileProperty {
    // ... 既存のフィールド
    
    /// ターゲット拡張（複数指定可能）
    pub target_extensions: Vec<TargetExtension>,
}
```

### CLI 引数の追加

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliTargetExt {
    Debug,
}

impl From<CliTargetExt> for TargetExtension {
    fn from(cli: CliTargetExt) -> Self {
        match cli {
            CliTargetExt::Debug => TargetExtension::Debug,
        }
    }
}

// Args 構造体に追加
#[derive(Parser, Debug)]
struct Args {
    // ... 既存のフィールド
    
    /// Target extensions (only with --mode=compile, can be specified multiple times)
    #[arg(long = "target-ext", value_enum)]
    target_ext: Vec<CliTargetExt>,
}
```

### バリデーション

- `target_extensions` は `mode=Compile` 時のみ有効
- `mode=Run` 時に指定されていた場合は警告またはエラー

## 実装手順

1. `src/compile_property.rs` の修正
   - `TargetExtension` 列挙型を追加
   - `CompileTarget::ExWs` を削除
   - `CompileProperty` に `target_extensions` フィールドを追加
   - バリデーションを更新

2. `src/bin/nospace20.rs` の修正
   - `CliTarget::ExWs` を削除
   - `CliTargetExt` 列挙型を追加
   - `Args` に `target_ext` フィールドを追加
   - `CompileProperty` 構築時に `target_extensions` を設定

3. テストの確認
   - 既存のテストが影響を受けないか確認
   - 必要に応じて新しいテストを追加

## 影響範囲

- `src/compile_property.rs`
- `src/bin/nospace20.rs`
- `README.md` (--help の出力が変わる)
- ドキュメント (`ai-docs/done-task/cli-compile-options.md`)

## 今後の拡張

`TargetExtension` に今後追加される可能性があるもの：
- `Trace` : トレース命令の埋め込み
- `Assert` : アサーション命令の埋め込み
- `Optimize` : 最適化拡張
- 等

## ノート

- 現時点では `debug` 拡張の具体的な動作は未定義
- 将来的に Whitespace の拡張命令として何らかのデバッグ機能を実装する際に使用される予定
