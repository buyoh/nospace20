//! `tools/nsrun.sh`（コンパイルして即時実行するラッパー）の結合テスト
//!
//! cargo がビルドした `nospace20` / `whitespace20` を環境変数で渡して、bash でスクリプトを実行する。
//! 入力のソースは既存のテストリソースを使い、ファイルは作らない。
//! `/dev/fd` と bash が必要なため unix のみ。

#![cfg(unix)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

/// `tools/nsrun.sh args...` を `stdin` を与えて実行する
fn nsrun(args: &[&str], stdin: &str) -> Output {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mut child = Command::new("bash")
        .arg(format!("{}/tools/nsrun.sh", manifest_dir))
        .args(args)
        .current_dir(manifest_dir)
        .env("NOSPACE20", env!("CARGO_BIN_EXE_nospace20"))
        .env("WHITESPACE20", env!("CARGO_BIN_EXE_whitespace20"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run bash");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// プログラムの stdin はスクリプトの stdin（ソースは別の fd で渡す）
#[test]
fn test_nsrun_passes_stdin_to_program() {
    let output = nsrun(
        &["resources/tests/passes/examples/e0-01-fibonacci.ns"],
        "10\n",
    );
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "89\n");
}

/// debug 拡張は既定で有効で、__assert の失敗は exit 1
#[test]
fn test_nsrun_debug_ext_is_enabled_by_default() {
    let output = nsrun(&["resources/tests/fails/runtime/assert_fail_001.ns"], "");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("assertion failed"),
        "{}",
        stderr_of(&output)
    );
}

/// --ignore-debug では debug 組み込み関数を無視する
#[test]
fn test_nsrun_ignore_debug() {
    let output = nsrun(
        &[
            "--ignore-debug",
            "resources/tests/fails/runtime/assert_fail_001.ns",
        ],
        "",
    );
    assert!(output.status.success(), "{}", stderr_of(&output));
}

/// コンパイルエラーでは VM を起動せず exit 1（空のプログラムを実行して exit 0 にしない）
#[test]
fn test_nsrun_compile_error_exits_with_failure() {
    let output = nsrun(
        &["resources/tests/fails/syntax/array_empty_init_list_001.ns"],
        "",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("error:"),
        "{}",
        stderr_of(&output)
    );
    assert_eq!(stdout_of(&output), "");
}

/// `--` より後の引数は whitespace20 に渡す
#[test]
fn test_nsrun_passes_args_after_separator_to_whitespace20() {
    let output = nsrun(
        &[
            "resources/tests/passes/examples/e0-01-fibonacci.ns",
            "--",
            "--max-steps",
            "10",
        ],
        "10\n",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("Execution limit exceeded (10 steps)"),
        "{}",
        stderr_of(&output)
    );
}
