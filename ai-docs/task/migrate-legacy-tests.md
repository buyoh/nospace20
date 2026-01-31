# 旧テストの移行計画

## 概要

`.local/nospace/test/` にある旧実装のテストケースを `resources/tests/` に移行し、テストハーネスを拡張する計画。

旧テストは以下の特徴を持つ：
- 標準入力 (`*_stdin.txt`) と期待される標準出力 (`*_expected.txt`) による検証
- `__puti`, `__putc`, `__geti`, `__getc` などのI/Oビルトイン関数を使用
- 一部は配列、ポインタ、論理演算子など未実装機能を使用

**方針**: I/O ビルトイン関数の実装を優先し、他の未実装機能を使用するテストはコメントアウトで対応する。

## 1. resources/tests の設計方針

### 1.1 ディレクトリ構成

```
resources/tests/
├── passes/
│   ├── io/                    # 新規：I/Oを使用するテスト
│   │   ├── stdio_001.ns
│   │   ├── stdio_001.check.json
│   │   ├── stdio_001.stdin     # 標準入力（オプション）
│   │   ├── stdio_002.ns
│   │   ...
│   ├── integration/           # 既存：統合テスト
│   │   ├── legacy_001.ns      # 旧テストからの移行
│   │   ...
│   └── [other categories]/
└── fails/
```

### 1.2 新しい check.json フォーマット

現在の `TestConfig` を拡張し、I/Oテストをサポート：

```json
{
  "type": "success_io",
  "stdout": "expected output here\n",
  "stdin": "optional input\n"
}
```

または、外部ファイル参照：

```json
{
  "type": "success_io",
  "stdout_file": "stdio_001.stdout",
  "stdin_file": "stdio_001.stdin"
}
```

### 1.3 テストファイルの分離について

**結論: 分離不要**

現状の `tests/code_test.rs` は以下の理由で1ファイルのまま維持する：
- テスト数が管理可能な規模
- マクロによる統一的な記述
- テストタイプ（success, success_io, parse_error）は同一ハーネスで処理可能

将来的にテスト数が大幅に増加した場合は、以下の分離を検討：
- `tests/io_test.rs` - I/O テスト専用
- `tests/syntax_test.rs` - 構文エラーテスト専用

## 2. I/O の実装設計

### 2.1 Read/Write トレイトを使用したパイプ設計

アプリケーションとしても標準入力から読み取り可能にするため、`std::io::Read` / `std::io::Write` トレイトを使用：

```rust
use std::io::{Read, Write, BufRead, BufReader};

pub struct Environment<R: Read, W: Write> {
    pub traced: BTreeMap<i64, i64>,
    pub stdin: BufReader<R>,
    pub stdout: W,
}

impl Environment<std::io::Stdin, std::io::Stdout> {
    /// 本番用：実際の標準入出力を使用
    pub fn new_stdio() -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin: BufReader::new(std::io::stdin()),
            stdout: std::io::stdout(),
        }
    }
}

impl<R: Read, W: Write> Environment<R, W> {
    /// テスト用：バッファを使用
    pub fn new_with_buffers(stdin: R, stdout: W) -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin: BufReader::new(stdin),
            stdout,
        }
    }
    
    pub fn write_int(&mut self, val: i64) {
        write!(self.stdout, "{}", val).unwrap();
    }
    
    pub fn write_char(&mut self, val: u8) {
        self.stdout.write_all(&[val]).unwrap();
    }
    
    pub fn read_int(&mut self) -> i64 {
        // 空白・改行をスキップして整数を読み取る
        let mut buf = String::new();
        // ... 実装
    }
    
    pub fn read_char(&mut self) -> i64 {
        let mut buf = [0u8; 1];
        match self.stdin.read(&mut buf) {
            Ok(1) => buf[0] as i64,
            _ => 0, // EOF または エラー
        }
    }
}
```

### 2.2 型パラメータの扱い

インタプリタ全体で型パラメータを使うと複雑になるため、以下の選択肢がある：

**選択肢A: Box<dyn Read/Write> を使用（推奨）**

```rust
pub struct Environment {
    pub traced: BTreeMap<i64, i64>,
    stdin: Box<dyn BufRead>,
    stdout: Box<dyn Write>,
}
```

メリット: 型パラメータが不要、コードがシンプル
デメリット: 動的ディスパッチのオーバーヘッド（微小）

**選択肢B: Cursor<Vec<u8>> をテスト用に使用**

```rust
// テスト用
let stdin = std::io::Cursor::new(b"42\n".to_vec());
let stdout = Vec::<u8>::new();
let env = Environment::new_with_buffers(stdin, stdout);
```

### 2.3 lib.rs の変更

```rust
/// 本番用：実際の標準入出力を使用
pub fn interpret_func(scope: &Scope, func_name: &str) -> Option<i64> {
    let mut env = Environment::new_stdio();
    interpret_func_impl(scope, func_name, &mut env)
}

/// テスト用：I/O バッファを指定
pub fn interpret_func_with_io(
    scope: &Scope,
    func_name: &str,
    stdin: &str
) -> (BTreeMap<i64, i64>, String) {
    let stdin_cursor = std::io::Cursor::new(stdin.as_bytes().to_vec());
    let mut stdout_buf = Vec::<u8>::new();
    let mut env = Environment::new_with_buffers(stdin_cursor, &mut stdout_buf);
    
    let _ = interpret_func_impl(scope, func_name, &mut env);
    
    (env.traced, String::from_utf8(stdout_buf).unwrap())
}
```

## 3. 追加で実装するビルトイン関数

| 関数 | 説明 | 優先度 |
|------|------|--------|
| `__puti(val)` | 整数を10進数で標準出力に出力。`val` を返す | 高 |
| `__putc(val)` | 文字（ASCII値）を標準出力に出力。`val` を返す | 高 |
| `__geti()` | 標準入力から整数を読み込み、値を返す | 高 |
| `__getc()` | 標準入力から1文字読み込み、ASCII値を返す | 高 |

`__getiv(addr)`, `__getcv()` は配列・ポインタ依存のため、現時点では実装しない。

## 4. tests/code_test.rs の変更

### 4.1 TestConfig enum の拡張

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum TestConfig {
    Success {
        trace: Vec<i64>,
    },
    SuccessIo {
        #[serde(default)]
        stdin: Option<String>,
        #[serde(default)]
        stdin_file: Option<String>,
        stdout: Option<String>,
        stdout_file: Option<String>,
    },
    ParseError {
        phase: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_count: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
}
```

### 4.2 新しいテスト実行関数

```rust
fn test_ok_coding_io_base(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")?;
    
    let check_json: TestConfig = /* 読み込み */;
    
    match check_json {
        TestConfig::SuccessIo { stdin, stdin_file, stdout, stdout_file } => {
            // stdin を取得（インラインまたはファイルから）
            let stdin_content = stdin.or_else(|| {
                stdin_file.map(|f| fs::read_to_string(path_base + &f).ok()).flatten()
            }).unwrap_or_default();
            
            // 期待される stdout を取得
            let expected_stdout = stdout.or_else(|| {
                stdout_file.map(|f| fs::read_to_string(path_base + &f).ok()).flatten()
            }).unwrap();
            
            // 実行
            let t = parse_to_tokens(&ns_cnt)?;
            let s = parse_to_tree(&t)?;
            let a = syntactic_analyze(&s);
            let (_, actual_stdout) = interpret_func_with_io(&a, "main", &stdin_content);
            
            assert_eq!(expected_stdout, actual_stdout);
        }
        _ => panic!("Expected success_io test config"),
    }
    Ok(())
}
```

### 4.3 マクロの追加

```rust
macro_rules! test_ok_coding_io {
    ($name: ident, $test_name: expr) => {
        #[test]
        fn $name() -> Result {
            test_ok_coding_io_base($test_name)
        }
    };
}

// I/O テスト
test_ok_coding_io!(test_io_stdio_001, "io/stdio_001");
test_ok_coding_io!(test_io_stdio_002, "io/stdio_002");
// ...
```

## 5. 旧テストの移行可能性分析（更新版）

### 5.1 Phase 1: I/O ビルトイン実装後に移行可能

以下のテストは I/O ビルトイン関数のみで動作：

| 旧テスト | 使用機能 | 移行先 | 状態 |
|----------|----------|--------|------|
| 001 | `__puti`, `__putc` | `io/puti_001` | ✅ 移行可能 |
| 002 | `__puti` | `io/puti_002` | ✅ 移行可能 |
| 003 | `__puti`, 変数 | `io/puti_003` | ✅ 移行可能 |
| 004 | `__puti`, 関数呼び出し | `io/func_puti_001` | ✅ 移行可能 |
| 005 | `__puti`, ローカル変数 | `io/local_var_001` | ✅ 移行可能 |
| 007 | `__puti`, while | `io/while_puti_001` | ✅ 移行可能 |
| 008 | `__puti`, while, if | `io/control_puti_001` | ✅ 移行可能 |
| 009 | `__puti`, 再帰 | `io/recursion_001` | ✅ 移行可能 |
| 010-012 | `__geti`, if | `io/geti_*` | ✅ 移行可能 |

### 5.2 未実装機能が必要（コメントアウトで対応）

| 旧テスト | 必要機能 | 対応方針 |
|----------|----------|----------|
| 006 | グローバル変数 | コメントアウト |
| 013-016 | ポインタ (`*`, `&`) | コメントアウト |
| 017-020 | 論理演算子 (`&&`, `||`, `!`) | コメントアウト |
| 021-024 | 配列 (`arr[n]`) | コメントアウト |
| 025 | 文字リテラル (`'0'`) | コメントアウト |
| 026-027 | 複合機能 | 分析後判断 |
| yukicoder* | 配列、ポインタ、複合代入 | コメントアウト |

## 6. 実装順序（改訂版）

### Phase 1: I/O ビルトイン実装（最優先）

1. `Environment` 構造体の拡張（`Box<dyn BufRead>`, `Box<dyn Write>`）
2. `__puti`, `__putc`, `__geti`, `__getc` の実装
3. `lib.rs` に `interpret_func_with_io` 追加
4. テストハーネスの拡張（`SuccessIo` 対応）

### Phase 2: 旧テストの移行

1. 旧テスト 001-005, 007-009 の移行（出力のみ）
2. 旧テスト 010-012 の移行（入力あり）
3. 未実装機能を使用するテストはコメントアウトで登録

### Phase 3 以降（将来）

以下は優先度低として後回し：
- 論理演算子 (`&&`, `||`, `!`)
- 文字リテラル (`'a'`)
- グローバル変数
- 配列・ポインタ
- 複合代入演算子

## 7. 関連ファイル

- [src/interpreter/mod.rs](../../src/interpreter/mod.rs) - インタプリタ実装
- [src/lib.rs](../../src/lib.rs) - 公開API
- [tests/code_test.rs](../../tests/code_test.rs) - テストハーネス
- [spec.md](../../spec.md) - 言語仕様
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md) - 実装状況

## 8. 進捗

### 2026-01-31

- [x] 計画立案・ドキュメント作成
- [x] spec.md に不足仕様を追記
- [ ] Phase 1: I/O ビルトイン実装
- [ ] Phase 2: 旧テストの移行
