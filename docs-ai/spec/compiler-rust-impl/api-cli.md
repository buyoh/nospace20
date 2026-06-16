# 公開 API・CLI 統合

## 公開 API

### lib.rs への追加

```rust
// src/lib.rs

pub mod compiler_ws;

pub use compiler_ws::{compile, CompileError, WsProgram};

/// Whitespace コードにコンパイル
pub fn compile_to_whitespace(scope: &Scope) -> Result<String, compiler_ws::CompileError> {
    let program = compiler_ws::compile(scope)?;
    Ok(program.to_whitespace())
}

/// デバッグ用の可読形式でコンパイル
pub fn compile_to_whitespace_debug(scope: &Scope) -> Result<String, compiler_ws::CompileError> {
    let program = compiler_ws::compile(scope)?;
    Ok(program.to_debug_string())
}
```

### 使用例

```rust
use nospace20::{parse_to_tokens, parse_to_tree, syntactic_analyze, compile_to_whitespace};

fn main() {
    let source = r#"
        func: main() {
            let: x(42);
            __puti(x);
        }
    "#.to_string();
    
    // パース
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast);
    
    // コンパイル
    match compile_to_whitespace(&scope) {
        Ok(ws_code) => {
            // ws_code は空白文字のみの文字列
            println!("Compiled {} bytes", ws_code.len());
        }
        Err(e) => {
            eprintln!("Compile error: {:?}", e);
        }
    }
}
```

## CLI 統合

### 出力モード

```rust
// src/bin/nospace20.rs

/// 出力モード
enum OutputMode {
    /// インタプリタ実行
    Run,
    /// Whitespace 出力
    CompileWs,
    /// 可読形式出力（デバッグ用）
    CompileWsDebug,
}
```

### コマンドライン引数

```
USAGE:
    nospace20 [OPTIONS] <FILE>

OPTIONS:
    -r, --run           インタプリタで実行（デフォルト）
    -c, --compile       Whitespace コードを出力
    -d, --debug         デバッグ用可読形式で出力
    -o, --output <FILE> 出力ファイル（省略時は stdout）
    -h, --help          ヘルプを表示
```

### main 関数の拡張

```rust
fn main() {
    let args = parse_args();
    
    // ソース読み込み
    let source = std::fs::read_to_string(&args.input_file)
        .expect("Failed to read source file");
    
    // パース
    let tokens = parse_to_tokens(&source)
        .expect("Failed to tokenize");
    let ast = parse_to_tree(&tokens)
        .expect("Failed to parse");
    let scope = syntactic_analyze(&ast);
    
    // モードに応じた処理
    match args.mode {
        OutputMode::Run => {
            let result = interpret_func(&scope, "main");
            if let Some(value) = result {
                println!("{}", value);
            }
        }
        
        OutputMode::CompileWs => {
            match compile_to_whitespace(&scope) {
                Ok(ws_code) => {
                    output(&args.output_file, &ws_code);
                }
                Err(e) => {
                    eprintln!("Compile error: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        
        OutputMode::CompileWsDebug => {
            match compile_to_whitespace_debug(&scope) {
                Ok(debug_code) => {
                    output(&args.output_file, &debug_code);
                }
                Err(e) => {
                    eprintln!("Compile error: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn output(path: &Option<String>, content: &str) {
    match path {
        Some(p) => {
            std::fs::write(p, content).expect("Failed to write output");
        }
        None => {
            print!("{}", content);
        }
    }
}
```

## エラーハンドリング

### CompileError 型

```rust
/// コンパイルエラー
#[derive(Debug, Clone)]
pub enum CompileError {
    /// 未定義の変数
    UndefinedVariable(String),
    
    /// 未定義の関数
    UndefinedFunction(String),
    
    /// main 関数が見つからない
    MainNotFound,
    
    /// 無効な操作
    InvalidOperation(String),
    
    /// 未サポートの機能
    Unsupported(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UndefinedVariable(name) => 
                write!(f, "Undefined variable: {}", name),
            CompileError::UndefinedFunction(name) => 
                write!(f, "Undefined function: {}", name),
            CompileError::MainNotFound => 
                write!(f, "Function 'main' not found"),
            CompileError::InvalidOperation(op) => 
                write!(f, "Invalid operation: {}", op),
            CompileError::Unsupported(feature) => 
                write!(f, "Unsupported feature: {}", feature),
        }
    }
}

impl std::error::Error for CompileError {}
```

## デバッグ出力形式

`to_debug_string()` の出力例：

```
Push(5)
Push(3)
Add
Duplicate
OutputNumber
Label(LabelId(16))
JumpIfZero(LabelId(17))
Push(1)
Sub
Jump(LabelId(16))
Label(LabelId(17))
Exit
```

可読性向上のオプション：

```rust
impl WsProgram {
    /// インデント付きデバッグ形式
    pub fn to_debug_string_pretty(&self) -> String {
        let mut result = String::new();
        let mut indent = 0;
        
        for inst in &self.instructions {
            // Label は左寄せ
            if matches!(inst, Instruction::Label(_)) {
                indent = 0;
            }
            
            result.push_str(&"  ".repeat(indent));
            result.push_str(&format!("{:?}\n", inst));
            
            // Label の後はインデント
            if matches!(inst, Instruction::Label(_)) {
                indent = 1;
            }
        }
        
        result
    }
}
```
