//! Whitespace VM プロファイラ
//!
//! 指定されたテストケースを nospace → Whitespace にコンパイルし、
//! プロファイリングモードで実行して統計を YAML 形式（デフォルト）または JSON 形式で出力する。
//!
//! # 使い方
//! ```bash
//! # デフォルトのテストケースをプロファイル（YAML 出力）
//! cargo run --example ws_profiler
//!
//! # JSON 形式で出力
//! cargo run --example ws_profiler -- --json
//!
//! # 特定の .ns ファイルを指定
//! cargo run --example ws_profiler -- path/to/file.ns
//!
//! # std-ext オプションを指定（alloc 拡張を有効化）
//! cargo run --example ws_profiler -- --std-ext alloc
//!
//! # 言語サブセットを指定
//! cargo run --example ws_profiler -- --std ws
//! ```

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

use nospace20::{
    cli_utils::CliCompileArgs,
    whitespace::{ProfileStats, StepResult, WhitespaceVM},
    LanguageStd, TargetExtension,
};

/// Whitespace VM プロファイラ
#[derive(Parser, Debug)]
#[command(name = "ws_profiler")]
#[command(about = "Profile nospace programs compiled to Whitespace")]
struct Args {
    /// .ns files to profile (reads from profile-targets.yaml if not specified)
    files: Vec<String>,

    #[command(flatten)]
    compile: CliCompileArgs,

    /// Output in JSON format instead of YAML
    #[arg(long)]
    json: bool,
}

/// コンパイルオプション（run_profile に渡す）
struct CompileOptions {
    debug_ext: bool,
    alloc_ext: bool,
    opt_options: nospace20::OptimizationOptions,
    /// 言語サブセット（将来の利用のために保持。現在は compile_to_whitespace_with_options に渡す API がないため未使用）
    #[allow(dead_code)]
    std: LanguageStd,
}

impl CompileOptions {
    fn from_args(args: &Args) -> Self {
        let exts: Vec<TargetExtension> = args.compile.std_ext.iter().map(|e| (*e).into()).collect();
        Self {
            debug_ext: exts.contains(&TargetExtension::Debug),
            alloc_ext: exts.contains(&TargetExtension::Alloc),
            opt_options: args.compile.build_optimization_options(),
            std: args.compile.std.into(),
        }
    }
}

// ===== profile-targets.yaml 読み込み用構造体 =====

/// profile-targets.yaml のルート構造体
#[derive(Deserialize)]
struct ProfileTargets {
    targets: Vec<ProfileTarget>,
}

/// プロファイル対象の定義
#[derive(Deserialize)]
struct ProfileTarget {
    /// resources/tests/passes/ からの相対パス（拡張子なし）か、絶対パス
    path: String,
    #[allow(dead_code)]
    #[serde(default)]
    comment: Option<String>,
    /// stdin に渡す文字列。未指定時は check.json から取得
    #[serde(default)]
    stdin: Option<String>,
}

// ===== check.json 読み込み用構造体 =====

/// check.json のルート（複数形式に対応）
#[derive(Deserialize, Default)]
struct CheckJson {
    /// success_io 形式の cases リスト
    #[serde(default)]
    cases: Vec<CheckCase>,
    /// インラインの stdin (cases なし形式)
    #[serde(default)]
    stdin: Option<String>,
}

/// check.json の cases 要素
#[derive(Deserialize)]
struct CheckCase {
    #[serde(default)]
    stdin: Option<String>,
}

// ===== YAML 出力用構造体 =====

/// プロファイルレポート全体
#[derive(Serialize)]
struct ProfileReport {
    profiles: Vec<ProfileEntry>,
}

/// テストケース1件のプロファイル結果
#[derive(Serialize)]
struct ProfileEntry {
    name: String,
    source: String,
    compile_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution: Option<ExecutionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// 実行プロファイル
#[derive(Serialize)]
struct ExecutionProfile {
    result: String,
    total_steps: usize,
    instruction_counts: InstructionCountsYaml,
    memory: MemoryProfile,
    stack: StackProfile,
    program: ProgramProfile,
}

/// 命令別カウント（YAML 出力用）
#[derive(Serialize)]
struct InstructionCountsYaml {
    push: usize,
    duplicate: usize,
    copy: usize,
    swap: usize,
    discard: usize,
    add: usize,
    sub: usize,
    mul: usize,
    div: usize,
    modulo: usize,
    store: usize,
    retrieve: usize,
    label: usize,
    call: usize,
    jump: usize,
    jump_if_zero: usize,
    jump_if_negative: usize,
    #[serde(rename = "return")]
    return_count: usize,
    exit: usize,
    output_char: usize,
    output_number: usize,
    input_char: usize,
    input_number: usize,
}

/// ヒープ（メモリ）アクセス統計（YAML 出力用）
#[derive(Serialize)]
struct MemoryProfile {
    heap_store_range: Option<[i64; 2]>,
    heap_retrieve_range: Option<[i64; 2]>,
    heap_store_count: usize,
    heap_retrieve_count: usize,
    heap_unique_addresses: usize,
}

/// スタック深さ統計（YAML 出力用）
#[derive(Serialize)]
struct StackProfile {
    max_data_stack_depth: usize,
    max_call_stack_depth: usize,
}

/// プログラム静的情報（YAML 出力用）
#[derive(Serialize)]
struct ProgramProfile {
    /// コンパイル後の静的命令数
    instruction_count: usize,
    /// Whitespace テキストのバイト数
    whitespace_size: usize,
}

// ===== 定数 =====

const PROFILE_TARGETS_PATH: &str = "resources/tests/profile-targets.yaml";
const TESTS_PASSES_DIR: &str = "resources/tests/passes";
const MAX_STEPS: usize = 10_000_000;

// ===== エントリポイント =====

fn main() {
    let args = Args::parse();
    let compile_opts = CompileOptions::from_args(&args);

    let targets: Vec<ProfileTarget> = if !args.files.is_empty() {
        // コマンドライン引数で指定されたファイルをそのままターゲットとして扱う
        args.files
            .iter()
            .map(|s| ProfileTarget {
                path: s.clone(),
                comment: None,
                stdin: None,
            })
            .collect()
    } else {
        // デフォルト: profile-targets.yaml からロード
        let yaml_content =
            fs::read_to_string(PROFILE_TARGETS_PATH).expect("Failed to read profile-targets.yaml");
        let manifest: ProfileTargets =
            serde_yaml::from_str(&yaml_content).expect("Failed to parse profile-targets.yaml");
        manifest.targets
    };

    let mut profiles = Vec::new();
    for target in &targets {
        let entry = run_profile(target, &compile_opts);
        profiles.push(entry);
    }

    let report = ProfileReport { profiles };
    if args.json {
        // JSON 形式で出力
        let json = serde_json::to_string_pretty(&report).expect("Failed to serialize JSON");
        println!("{}", json);
    } else {
        // ヘッダーコメント付きで YAML 出力（デフォルト）
        println!("# Whitespace VM Profile Report");
        let yaml = serde_yaml::to_string(&report).expect("Failed to serialize YAML");
        print!("{}", yaml);
    }
}

// ===== プロファイル実行 =====

/// 1テストケースをプロファイルして結果を返す
fn run_profile(target: &ProfileTarget, opts: &CompileOptions) -> ProfileEntry {
    // --- ソースファイルのパスを解決 ---
    let (source_path, name) = resolve_source_path(&target.path);

    // --- ソースコードを読み込む ---
    let source_code = match fs::read_to_string(&source_path) {
        Ok(s) => s,
        Err(e) => {
            return ProfileEntry {
                name,
                source: source_path,
                compile_success: false,
                execution: None,
                error: Some(format!("Failed to read source: {}", e)),
            };
        }
    };

    // --- nospace をパース → コンパイル ---
    let ws_source = match compile_nospace(&source_code, opts) {
        Ok(ws) => ws,
        Err(e) => {
            return ProfileEntry {
                name,
                source: source_path,
                compile_success: false,
                execution: None,
                error: Some(format!("Compile error: {}", e)),
            };
        }
    };

    let instruction_count = count_instructions(&ws_source);
    let whitespace_size = ws_source.len();

    // --- stdin を解決（target 指定 > check.json > 空文字列）---
    let stdin_str = resolve_stdin(target, &source_path);

    // --- VM 構築・実行 ---
    let vm_result = WhitespaceVM::from_source(&ws_source);
    let mut vm = match vm_result {
        Ok(vm) => vm,
        Err(e) => {
            return ProfileEntry {
                name,
                source: source_path,
                compile_success: true,
                execution: None,
                error: Some(format!("VM parse error: {:?}", e)),
            };
        }
    };

    vm = vm
        .with_io(
            Box::new(std::io::BufReader::new(std::io::Cursor::new(
                stdin_str.into_bytes(),
            ))),
            Box::new(Vec::<u8>::new()),
        )
        .with_debug_ext(opts.debug_ext)
        .with_profiling(true);

    let step_result = vm.run(MAX_STEPS);

    let result_str = match &step_result {
        StepResult::Complete => "Complete".to_string(),
        StepResult::Suspended => "Suspended".to_string(),
        StepResult::Error(e) => format!("Error({:?})", e),
        StepResult::WaitingForInput(_) => "WaitingForInput".to_string(),
    };

    let stats: &ProfileStats = vm.profile_stats();
    let ic = &stats.instruction_counts;
    let heap = &stats.heap;
    let stack = &stats.stack;

    ProfileEntry {
        name,
        source: source_path,
        compile_success: true,
        execution: Some(ExecutionProfile {
            result: result_str,
            total_steps: vm.total_steps(),
            instruction_counts: InstructionCountsYaml {
                push: ic.push,
                duplicate: ic.duplicate,
                copy: ic.copy,
                swap: ic.swap,
                discard: ic.discard,
                add: ic.add,
                sub: ic.sub,
                mul: ic.mul,
                div: ic.div,
                modulo: ic.modulo,
                store: ic.store,
                retrieve: ic.retrieve,
                label: ic.label,
                call: ic.call,
                jump: ic.jump,
                jump_if_zero: ic.jump_if_zero,
                jump_if_negative: ic.jump_if_negative,
                return_count: ic.return_count,
                exit: ic.exit,
                output_char: ic.output_char,
                output_number: ic.output_number,
                input_char: ic.input_char,
                input_number: ic.input_number,
            },
            memory: MemoryProfile {
                heap_store_range: heap.store_range.map(|(a, b)| [a, b]),
                heap_retrieve_range: heap.retrieve_range.map(|(a, b)| [a, b]),
                heap_store_count: heap.store_count,
                heap_retrieve_count: heap.retrieve_count,
                heap_unique_addresses: heap.unique_address_count,
            },
            stack: StackProfile {
                max_data_stack_depth: stack.max_data_stack_depth,
                max_call_stack_depth: stack.max_call_stack_depth,
            },
            program: ProgramProfile {
                instruction_count,
                whitespace_size,
            },
        }),
        error: None,
    }
}

// ===== ヘルパー =====

/// ターゲットのパスから (ソースファイルの絶対/相対パス, 名前) を解決する
fn resolve_source_path(path: &str) -> (String, String) {
    // 引数が .ns で終わっていれば直接ファイルパスとして扱う
    if path.ends_with(".ns") {
        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string();
        return (path.to_string(), name);
    }

    // resources/tests/passes/ からの相対パスとして扱う
    let source_path = format!("{}/{}.ns", TESTS_PASSES_DIR, path);
    let name = path.to_string();
    (source_path, name)
}

/// nospace ソースコードを Whitespace テキストにコンパイルする
fn compile_nospace(source: &str, opts: &CompileOptions) -> Result<String, String> {
    let source_string = source.to_string();
    let tokens = nospace20::parse_to_tokens(&source_string).map_err(|errors| {
        errors
            .iter()
            .map(|e| format!("{:?}", e))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let tree = nospace20::parse_to_tree(&tokens).map_err(|errors| {
        errors
            .iter()
            .map(|e| format!("{:?}", e))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let mut scope = nospace20::syntactic_analyze(&tree).map_err(|errors| {
        errors
            .iter()
            .map(|e| format!("{:?}", e))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    if opts.opt_options.any_enabled() {
        nospace20::optimize(&mut scope, &opts.opt_options);
    }
    nospace20::compile_to_whitespace_with_opt(
        &scope,
        opts.debug_ext,
        opts.alloc_ext,
        &opts.opt_options,
    )
    .map_err(|errors| {
        errors
            .iter()
            .map(|e| format!("{:?}", e))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// Whitespace テキストの静的命令数を計算する（パースして命令列長を得る）
fn count_instructions(ws_source: &str) -> usize {
    match nospace20::whitespace::parse(ws_source) {
        Ok(instructions) => instructions.len(),
        Err(_) => 0,
    }
}

/// stdin 文字列を解決する
///
/// 優先順位: ProfileTarget.stdin > check.json の stdin > 空文字列
fn resolve_stdin(target: &ProfileTarget, source_path: &str) -> String {
    // 1. ターゲットに stdin が直接指定されている場合
    if let Some(ref s) = target.stdin {
        return s.clone();
    }

    // 2. check.json から取得を試みる
    let check_path = source_path.replace(".ns", ".check.json");
    if let Ok(content) = fs::read_to_string(&check_path) {
        if let Ok(check) = serde_json::from_str::<CheckJson>(&content) {
            // インライン stdin
            if let Some(ref s) = check.stdin {
                return s.clone();
            }
            // cases の最初の要素から stdin を取得
            if let Some(first_case) = check.cases.first() {
                if let Some(ref s) = first_case.stdin {
                    return s.clone();
                }
            }
        }
    }

    // 3. デフォルト: 空文字列
    String::new()
}
