# コンパイラテスト戦略

## 概要

nospace → Whitespace コンパイラのテストには、生成された Whitespace コードを実行して結果を検証する必要がある。外部の Whitespace インタプリタとして `whitespacers` (`wsc`) を使用する。

## ライセンス考慮事項

| プロジェクト | ライセンス |
|-------------|-----------|
| nospace20 | MIT |
| whitespacers | MPL-2.0 |

MPL-2.0 はファイル単位の copyleft ライセンスであるため、nospace20 本体とは独立してビルド・配置することでライセンスの分離を明確にする。

## 構成

### ディレクトリ構造

```
nospace20/
├── Cargo.toml              # nospace20 本体 (MIT)
├── tools/
│   ├── wsc-install/        # whitespacers インストール先 (.gitignore)
│   │   └── bin/
│   │       └── wsc         # Whitespace インタプリタ実行ファイル
│   ├── setup-wsc.sh        # インストールスクリプト
│   └── ...
├── tests/
│   └── compile_test.rs     # コンパイラ統合テスト
└── .gitignore              # /tools/wsc-install を追加
```

### インストール方式

`cargo install` の `--root` オプションを使用して、`tools/wsc-install/` にインストールする。

```bash
cargo install whitespacers --root ./tools/wsc-install
```

これにより以下が生成される:
- `tools/wsc-install/bin/wsc` - Whitespace インタプリタ実行ファイル
- `tools/wsc-install/.crates.toml` - インストール情報
- `tools/wsc-install/.crates2.json` - インストール情報

## 実装計画

### Phase 1: 環境セットアップ

#### 1.1 .gitignore の更新

```ignore
/tools/wsc-install
```

#### 1.2 セットアップスクリプトの作成

`tools/setup-wsc.sh`:
```bash
#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$SCRIPT_DIR/wsc-install"

echo "Installing whitespacers to $INSTALL_DIR ..."
cargo install whitespacers --root "$INSTALL_DIR"

echo "Done. wsc is available at: $INSTALL_DIR/bin/wsc"
echo "Version:"
"$INSTALL_DIR/bin/wsc" --version
```

#### 1.3 README の作成

`tools/wsc-install/README.md` (手動作成、.gitignore されない):
```markdown
# wsc (whitespacers)

このディレクトリには `whitespacers` (MPL-2.0) の実行ファイルがインストールされます。

## インストール方法

```bash
cd tools
./setup-wsc.sh
```

## ライセンス

whitespacers は MPL-2.0 License です。
https://github.com/CensoredUsername/whitespace-rs
```

### Phase 2: テストユーティリティ

#### 2.1 wsc パス解決

テストコードから `wsc` のパスを解決するユーティリティ:

```rust
// tests/common/mod.rs または src/test_utils.rs

use std::path::{Path, PathBuf};
use std::process::Command;

/// wsc 実行ファイルのパスを取得
/// 優先順位:
/// 1. tools/wsc-install/bin/wsc (プロジェクト内)
/// 2. PATH 上の wsc (グローバル)
pub fn find_wsc() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    
    // プロジェクト内の wsc を優先
    let project_wsc = manifest_dir.join("tools/wsc-install/bin/wsc");
    if project_wsc.exists() {
        return Some(project_wsc);
    }
    
    // Windows の場合は .exe も確認
    let project_wsc_exe = manifest_dir.join("tools/wsc-install/bin/wsc.exe");
    if project_wsc_exe.exists() {
        return Some(project_wsc_exe);
    }
    
    // グローバルの wsc を探す
    which_wsc()
}

/// PATH 上の wsc を探す
fn which_wsc() -> Option<PathBuf> {
    // Unix
    if let Ok(output) = Command::new("which").arg("wsc").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }
    
    // Windows
    if let Ok(output) = Command::new("where").arg("wsc").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string());
            if let Some(p) = path {
                if !p.is_empty() {
                    return Some(PathBuf::from(p));
                }
            }
        }
    }
    
    None
}

/// wsc が利用可能かチェック
pub fn wsc_available() -> bool {
    find_wsc().is_some()
}
```

#### 2.2 Whitespace 実行ヘルパー

```rust
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// Whitespace コードを実行して結果を取得
pub fn run_whitespace(ws_code: &str, stdin_input: &str) -> Result<String, String> {
    let wsc_path = find_wsc()
        .ok_or_else(|| "wsc not found. Run: ./tools/setup-wsc.sh".to_string())?;
    
    // 一時ファイルに Whitespace コードを書き出し
    let mut file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(ws_code.as_bytes())
        .map_err(|e| format!("Failed to write temp file: {}", e))?;
    
    // wsc で実行
    let mut child = Command::new(&wsc_path)
        .arg(file.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn wsc: {}", e))?;
    
    // stdin に入力を送信
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_input.as_bytes());
    }
    
    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to wait for wsc: {}", e))?;
    
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "wsc failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
```

### Phase 3: テスト実装

#### 3.1 コンパイラ統合テスト

```rust
// tests/compile_test.rs

mod common;

use common::{wsc_available, run_whitespace};

/// wsc がない場合はスキップするマクロ
macro_rules! require_wsc {
    () => {
        if !wsc_available() {
            eprintln!("Skipping test: wsc not available");
            eprintln!("Run: ./tools/setup-wsc.sh");
            return;
        }
    };
}

#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn test_compile_simple_return() {
    require_wsc!();
    
    let source = r#"
        func: main() {
            return: 42;
        }
    "#;
    
    let ws_code = compile_nospace_to_whitespace(source).unwrap();
    let output = run_whitespace(&ws_code, "").unwrap();
    
    // main の戻り値は直接出力されないため、
    // __puti を使うテストに変更するか、終了コードを検証
}

#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn test_compile_puti() {
    require_wsc!();
    
    let source = r#"
        func: main() {
            __puti(42);
        }
    "#;
    
    let ws_code = compile_nospace_to_whitespace(source).unwrap();
    let output = run_whitespace(&ws_code, "").unwrap();
    
    assert_eq!(output.trim(), "42");
}
```

### ~~Phase 4: CI/CD 統合~~

実装しない。

#### ~~4.1 GitHub Actions~~

実装しない。

## 使用方法

### 開発者向けセットアップ

```bash
# 1. リポジトリをクローン
git clone https://github.com/buyoh/nospace20.git
cd nospace20

# 2. wsc をインストール（コンパイラテストを実行する場合）
./tools/setup-wsc.sh

# 3. テスト実行
cargo test                    # 通常のテスト
cargo test -- --ignored       # wsc 依存テストも含む
```

### wsc の手動実行

```bash
# Whitespace ファイルを実行
./tools/wsc-install/bin/wsc path/to/file.ws

# 標準入力から入力を与える
echo "42" | ./tools/wsc-install/bin/wsc path/to/file.ws

# ヘルプ
./tools/wsc-install/bin/wsc --help
```

## 注意事項

1. `tools/wsc-install/` は `.gitignore` に追加されるため、各開発者が個別にセットアップする必要がある
2. CI/CD ではキャッシュを使用して毎回のビルドを回避する
3. whitespacers のバージョンを固定する場合は `cargo install whitespacers@1.3.0 --root ...` とする
4. Windows では `wsc.exe` が生成される

## 関連ドキュメント

- [whitespacers (crates.io)](https://crates.io/crates/whitespacers)
- [whitespacers (GitHub)](https://github.com/CensoredUsername/whitespace-rs)
- [Whitespace 仕様](../../spec-whitespace.md)
