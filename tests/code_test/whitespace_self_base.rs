use std::{fs, io};

use nospace20::whitespace::{StepResult, WhitespaceVM};
use nospace20::{
    compile_to_ws, parse_to_tokens, parse_to_tree, semantic_analyze, OptimizationOptions,
    WsCompileOptions,
};

use super::test_config::{load_check_json, TestConfig};
use super::trace_assert::assert_trace_hit_counts;

/// `_ws_self` 系のテストでの実行ステップの上限
pub const WS_SELF_MAX_STEPS: usize = 1_000_000;

/// `_ws_self` 系のテスト（compile → 独自 WhitespaceVM）の実行条件
///
/// build.rs が生成するテストコードから、manifest の設定とターゲットに応じた値で渡される。
#[derive(Debug, Clone, Copy)]
pub struct WsSelfTarget {
    /// debug 拡張ありでコンパイル・実行する。true のとき success の `trace_hit_counts` を照合する
    pub debug_ext: bool,
    /// alloc 拡張ありでコンパイルする
    pub alloc_ext: bool,
    /// 未初期化ヒープの読み出しをエラーにする
    pub strict_heap: bool,
    /// 未初期化ヒープの読み出しでランダム値を返す（初期値 0 依存のバグの検出用）
    pub randomize_heap: bool,
    /// `--opt all` 相当の最適化を適用してからコンパイルする
    pub opt_all: bool,
}

impl WsSelfTarget {
    /// 失敗時のメッセージに使うターゲットの名前
    fn label(&self) -> String {
        format!("ws_self {:?}", self)
    }
}

/// ソースを `target` の条件で Whitespace にコンパイルする
///
/// 解析・コンパイルに失敗したら panic する（テストの失敗）
pub fn compile_for_ws_self(source: &str, target: WsSelfTarget) -> String {
    let t = parse_to_tokens(&source.to_string()).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let mut a = semantic_analyze(&s).unwrap();
    if target.opt_all {
        nospace20::optimize(&mut a, &OptimizationOptions::all());
    }
    let ws_code = compile_to_ws(
        &a,
        &WsCompileOptions {
            debug_ext: target.debug_ext,
            alloc_ext: target.alloc_ext,
            optimization: if target.opt_all {
                OptimizationOptions::all()
            } else {
                OptimizationOptions::none()
            },
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

    // Whitespace コードが空白文字のみであることを確認
    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );
    ws_code
}

/// コンパイル済みの Whitespace から、`target` の条件の VM を作る（stdin は空）
pub fn new_ws_self_vm(ws_code: &str, target: WsSelfTarget, test_name: &str) -> WhitespaceVM {
    WhitespaceVM::from_source(ws_code)
        .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e))
        .with_debug_ext(target.debug_ext)
        .with_strict_heap(target.strict_heap)
        .with_randomize_heap(target.randomize_heap)
}

/// 実行結果が正常終了でなければ panic する
fn expect_complete(result: StepResult, test_name: &str, case_name: Option<&str>) {
    let context = match case_name {
        Some(case) => format!("{}, case '{}'", test_name, case),
        None => test_name.to_string(),
    };
    match result {
        StepResult::Complete => {}
        StepResult::Suspended => panic!(
            "Whitespace execution suspended (exceeded step limit) for {}",
            context
        ),
        StepResult::Error(e) => panic!("Whitespace execution failed for {}: {:?}", context, e),
        StepResult::WaitingForInput(t) => panic!(
            "Whitespace execution unexpectedly waiting for input ({:?}) for {}",
            t, context
        ),
    }
}

/// success テスト: 完走すること。debug 拡張ありなら `trace_hit_counts` も照合する
pub fn test_whitespace_self_base(test_name: &str, target: WsSelfTarget) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let ws_code = compile_for_ws_self(&ns_cnt, target);
    let mut vm = new_ws_self_vm(&ws_code, target, test_name);
    let result = vm.run(WS_SELF_MAX_STEPS);
    expect_complete(result, test_name, None);

    // debug 拡張なしでは __trace が記録されないので照合しない
    if target.debug_ext {
        match load_check_json(&path_base) {
            TestConfig::Success { trace_hit_counts } => assert_trace_hit_counts(
                &trace_hit_counts,
                &vm.traced,
                &format!("{} {}", test_name, target.label()),
            ),
            _ => panic!("Expected success test config"),
        }
    }
}

/// success_io テスト: 各ケースの stdin を与えて実行し、stdout を照合する
pub fn test_whitespace_self_io_base(test_name: &str, target: WsSelfTarget) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let check_json_value: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();

    let check_json: TestConfig = serde_json::from_value(check_json_value).ok().unwrap();
    let test_cases = check_json.get_io_test_cases();

    // コンパイル（全ケース共通）
    let ws_code = compile_for_ws_self(&ns_cnt, target);

    // 各ケースを実行
    for (idx, case) in test_cases.iter().enumerate() {
        let case_name = case
            .name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format!("case_{}", idx));

        // stdin を取得
        let stdin_content = if let Some(s) = &case.stdin {
            s.clone()
        } else if let Some(f) = &case.stdin_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap_or_default()
        } else {
            String::new()
        };

        // 期待される stdout を取得
        let expected_stdout = if let Some(s) = &case.stdout {
            s.clone()
        } else if let Some(f) = &case.stdout_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap()
        } else {
            panic!(
                "IoTestCase must specify stdout or stdout_file (test: {}, case: {})",
                test_name, case_name
            );
        };

        // 独自 WhitespaceVM で実行
        let stdin_cursor: Box<dyn io::BufRead> =
            Box::new(io::Cursor::new(stdin_content.into_bytes()));
        let mut vm = new_ws_self_vm(&ws_code, target, test_name).with_stdin(stdin_cursor);

        let result = vm.run(WS_SELF_MAX_STEPS);
        expect_complete(result, test_name, Some(&case_name));

        let actual_stdout = vm.get_stdout_string();

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'\nExpected: {:?}\nActual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }
}
