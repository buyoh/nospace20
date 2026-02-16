# エラー仕様の自動生成手段

## 目的

ソースコードからエラーメッセージの仕様を自動的に抽出し、以下を実現する：

- ドキュメントとコードの同期を保つ
- テストケースの網羅性を機械的に検証
- ユーザー向けエラーリファレンスを自動生成

## 現状分析

### エラーメッセージの実装パターン

#### 1. 静的文字列リテラル

```rust
code_parse_error!(
    hex_idx,
    "invalid hexadecimal literal: expected at least one hex digit after '0x'"
)
```

**特徴**:
- 文字列リテラルとして直接埋め込み
- grep で簡単に抽出可能

#### 2. フォーマット文字列

```rust
code_parse_error!(format!("undefined variable: {}", v))
```

**特徴**:
- 動的な情報を含む
- テンプレートとして抽出する必要がある

#### 3. enum のバリアント

```rust
pub enum CompileError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}
```

**特徴**:
- Display トレイトで文字列化
- enum 定義と Display 実装を両方パースする必要がある

---

## 提案される自動生成手段

### 手段1: 静的解析ツール（Rust マクロ + build.rs）

#### 概要

Rust のマクロシステムを利用して、コンパイル時にエラーメッセージを収集する。

#### 実装方法

1. **専用マクロの定義**

```rust
// src/base/error_registry.rs
#[macro_export]
macro_rules! register_error {
    ($category:expr, $code:expr, $message:expr) => {
        {
            #[cfg(feature = "error-doc-gen")]
            {
                inventory::submit! {
                    ErrorSpec {
                        category: $category,
                        code: $code,
                        message: $message,
                    }
                }
            }
            CodeParseError::new(None, $message)
        }
    };
}
```

2. **build.rs でエラー仕様を収集**

```rust
// build.rs
fn generate_error_docs() {
    // inventory クレートを使用してエラー仕様を収集
    // JSON または Markdown として出力
}
```

#### メリット

- ソースコードとドキュメントの同期が保証される
- コンパイル時にエラー一覧を自動生成
- タイプセーフ

#### デメリット

- 既存コードの大規模な書き換えが必要
- コンパイル時間の増加

---

### 手段2: 正規表現ベースの静的解析

#### 概要

ソースコードを grep や正規表現で解析し、エラーメッセージを抽出する。

#### 実装方法

スクリプト例（Python）:

```python
import re
import json
from pathlib import Path

def extract_errors(source_file):
    errors = []
    content = source_file.read_text()
    
    # パターン1: code_parse_error! マクロの文字列リテラル
    pattern1 = r'code_parse_error!\([^,)]*,\s*"([^"]+)"'
    for match in re.finditer(pattern1, content):
        errors.append({
            "type": "parse_error",
            "message": match.group(1),
            "file": str(source_file),
            "line": content[:match.start()].count('\n') + 1
        })
    
    # パターン2: format! を使ったエラーメッセージ
    pattern2 = r'code_parse_error!\(format!\("([^"]+)"'
    for match in re.finditer(pattern2, content):
        errors.append({
            "type": "parse_error",
            "message_template": match.group(1),
            "file": str(source_file),
            "line": content[:match.start()].count('\n') + 1
        })
    
    return errors

def main():
    src_dir = Path("src")
    all_errors = []
    
    for rs_file in src_dir.rglob("*.rs"):
        all_errors.extend(extract_errors(rs_file))
    
    # JSON として出力
    with open("error-catalog.json", "w") as f:
        json.dump(all_errors, f, indent=2)
    
    # Markdown として出力
    generate_markdown(all_errors)

def generate_markdown(errors):
    # エラーカテゴリごとにグループ化してMarkdownを生成
    pass
```

#### メリット

- 実装が簡単
- 既存コードの変更不要
- 軽量・高速

#### デメリット

- 誤検出・検出漏れの可能性
- 複雑なパターンに対応しづらい
- コードとの同期は手動管理

---

### 手段3: syn クレートを使った本格的なパーサー

#### 概要

Rust の構文解析ライブラリ `syn` を使用して、ソースコードを完全にパースする。

#### 実装方法

```rust
// tools/error-extractor/src/main.rs
use syn::{File, Expr, Macro};
use quote::ToTokens;

fn extract_error_from_macro(mac: &Macro) -> Option<String> {
    let path = mac.path.segments.last()?.ident.to_string();
    if path != "code_parse_error" {
        return None;
    }
    
    // マクロの引数を解析
    let tokens = &mac.tokens;
    // ... トークン列から文字列リテラルを抽出
    
    Some(extracted_message)
}

fn main() {
    for file in find_rust_files("src") {
        let content = std::fs::read_to_string(file)?;
        let syntax_tree: File = syn::parse_file(&content)?;
        
        // 構文木を走査してエラーメッセージを抽出
        // ...
    }
}
```

#### メリット

- 高精度な解析
- 複雑なパターンにも対応可能
- enum の Display 実装なども解析可能

#### デメリット

- 実装コストが高い
- メンテナンスコスト

---

## 推奨アプローチ

### フェーズ1: 簡易的な正規表現ベース抽出（短期）

**実装内容**:
- Python または Rust でシンプルな抽出スクリプトを作成
- `code_parse_error!` マクロ呼び出しから文字列を抽出
- JSON ファイルとして出力

**成果物**:
- `error-catalog.json` - 全エラーメッセージのカタログ
- `docs/errors.md` - 自動生成されたエラーリファレンス

**工数**: 1-2日

---

### フェーズ2: テストカバレッジの検証（中期）

**実装内容**:
- エラーカタログと test-manifest.yaml を照合
- カバーされていないエラーを特定
- 不足テストケースのレポート生成

**成果物**:
- `error-coverage-report.md` - カバレッジレポート
- 不足しているテストケースのリスト

**工数**: 2-3日

---

### フェーズ3: enum エラーの自動抽出（長期）

**実装内容**:
- `CompileError`, `RuntimeError` などの enum を解析
- Display トレイトの実装を解析してメッセージを抽出

**ツール**: syn クレートを使用

**成果物**:
- 統合されたエラーカタログ（全フェーズのエラーを含む）

**工数**: 3-5日

---

## スクリプト実装例（フェーズ1）

### tools/extract-errors.py

```python
#!/usr/bin/env python3
"""
nospace エラーメッセージ抽出ツール

ソースコードから code_parse_error! マクロ呼び出しを抽出し、
エラーメッセージのカタログを生成します。
"""

import re
import json
from pathlib import Path
from typing import List, Dict
from dataclasses import dataclass, asdict

@dataclass
class ErrorEntry:
    category: str
    message: str
    message_template: str
    file: str
    line: int
    is_format: bool

def categorize_file(file_path: Path) -> str:
    """ファイルパスからエラーカテゴリを判定"""
    parts = file_path.parts
    if "token_parser" in parts:
        return "tokenize"
    elif "tree_parser" in parts:
        return "parse"
    elif "semantic_analyzer" in parts:
        return "semantic"
    elif "compiler_ws" in parts:
        return "compile"
    elif "whitespace" in parts:
        if "interpreter" in parts:
            return "runtime"
        else:
            return "whitespace_parse"
    return "unknown"

def extract_errors(source_file: Path) -> List[ErrorEntry]:
    """単一のソースファイルからエラーメッセージを抽出"""
    errors = []
    content = source_file.read_text(encoding="utf-8")
    category = categorize_file(source_file)
    
    # パターン1: 文字列リテラル
    # code_parse_error!(ptr, "message")
    pattern1 = r'code_parse_error!\([^,)]*,\s*"([^"]+)"'
    for match in re.finditer(pattern1, content):
        line_num = content[:match.start()].count('\n') + 1
        message = match.group(1)
        errors.append(ErrorEntry(
            category=category,
            message=message,
            message_template="",
            file=str(source_file),
            line=line_num,
            is_format=False
        ))
    
    # パターン2: format! マクロ
    # code_parse_error!(format!("template {}", var))
    pattern2 = r'code_parse_error!\(format!\("([^"]+)"'
    for match in re.finditer(pattern2, content):
        line_num = content[:match.start()].count('\n') + 1
        template = match.group(1)
        errors.append(ErrorEntry(
            category=category,
            message="",
            message_template=template,
            file=str(source_file),
            line=line_num,
            is_format=True
        ))
    
    return errors

def main():
    src_dir = Path("src")
    all_errors = []
    
    print("エラーメッセージを抽出中...")
    for rs_file in src_dir.rglob("*.rs"):
        errors = extract_errors(rs_file)
        all_errors.extend(errors)
        if errors:
            print(f"  {rs_file}: {len(errors)} 件")
    
    print(f"\n合計 {len(all_errors)} 件のエラーメッセージを検出")
    
    # JSON として出力
    output_file = Path("error-catalog.json")
    with open(output_file, "w", encoding="utf-8") as f:
        json.dump([asdict(e) for e in all_errors], f, indent=2, ensure_ascii=False)
    
    print(f"\n結果を {output_file} に保存しました")
    
    # カテゴリ別の統計
    from collections import Counter
    category_counts = Counter(e.category for e in all_errors)
    print("\n【カテゴリ別統計】")
    for cat, count in category_counts.most_common():
        print(f"  {cat}: {count} 件")

if __name__ == "__main__":
    main()
```

### 使用方法

```bash
# プロジェクトルートで実行
cd /path/to/nospace20
python3 tools/extract-errors.py

# 出力: error-catalog.json
```

---

## 今後の展開

1. エラーカタログの継続的更新
   - CI/CD パイプラインに組み込む
   - コミット時に自動実行

2. ユーザー向けエラーリファレンスの生成
   - カタログから HTML/PDF ドキュメントを生成
   - エラーコードの導入（例: `E001`, `E002`）

3. 多言語対応
   - エラーメッセージの翻訳データベース
   - ロケールに応じたメッセージ切り替え

4. IDE 統合
   - エラーメッセージにクイックフィックスを提案
   - 詳細なヘルプへのリンク
