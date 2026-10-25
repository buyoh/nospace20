//! CLI（`nospace20` / `whitespace20`）固有の挙動の結合テスト
//!
//! cargo がビルドしたバイナリを起動し、ソース・入力は stdin のパイプで渡す（ファイルは作らない）。
//! 実行の本体（`runner`）はユニットテストで確かめるため、ここでは引数の解釈・終了コード・表示だけを見る。

#![cfg(unix)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

use nospace20::{compile_source, WsCompileOptions};

/// `bin args...` を `stdin` を与えて実行する。`stdout` を指定しなければパイプで受け取る
fn run_bin(bin: &str, args: &[&str], stdin: &str, stdout: Option<Stdio>) -> Output {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(stdout.unwrap_or_else(Stdio::piped))
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run binary");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn nospace20(args: &[&str], stdin: &str, stdout: Option<Stdio>) -> Output {
    run_bin(env!("CARGO_BIN_EXE_nospace20"), args, stdin, stdout)
}

fn whitespace20(args: &[&str], stdin: &str, stdout: Option<Stdio>) -> Output {
    run_bin(env!("CARGO_BIN_EXE_whitespace20"), args, stdin, stdout)
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// 書き込みが常に失敗する出力先（`/dev/full`。既存のデバイスを開くだけでファイルは作らない）
#[cfg(target_os = "linux")]
fn dev_full() -> Stdio {
    Stdio::from(
        std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap(),
    )
}

fn compile_debug(source: &str) -> String {
    compile_source(
        source,
        &WsCompileOptions {
            debug_ext: true,
            ..Default::default()
        },
    )
    .unwrap()
}

// ===== nospace20 =====

/// 引数なしでは stdin のソースをコンパイルし、Whitespace を stdout に出す（実行はしない）。
/// alloc 拡張は既定で有効
#[test]
fn test_nospace20_compiles_to_stdout() {
    let source = "func: __main() { __puti(42); }";
    let output = nospace20(&[], source, None);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(
        stdout_of(&output),
        compile_source(
            source,
            &WsCompileOptions {
                alloc_ext: true,
                ..Default::default()
            }
        )
        .unwrap()
    );
}

#[test]
fn test_nospace20_missing_main_is_error() {
    let output = nospace20(&[], "func: f() { }", None);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("__main function not found"),
        "{}",
        stderr_of(&output)
    );
}

/// stdout への書き込みの失敗は exit 1（従来は `print!` が panic した）
#[cfg(target_os = "linux")]
#[test]
fn test_nospace20_stdout_write_failure_is_error() {
    let output = nospace20(&[], "func: __main() { __puti(42); }", Some(dev_full()));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("failed to write to stdout"),
        "{}",
        stderr_of(&output)
    );
}

// ===== whitespace20 =====

/// `-` はソースを stdin から読む。プログラムの stdin はそのあと空（EOF）になる
#[test]
fn test_whitespace20_reads_source_from_stdin() {
    let ws = compile_debug("func: __main() { __puti(42); }");
    let output = whitespace20(&["-"], &ws, None);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "42");

    let ws = compile_debug("func: __main() { __puti(__geti()); }");
    let output = whitespace20(&["-"], &ws, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("Runtime error: I/O error: unexpected end of input"),
        "{}",
        stderr_of(&output)
    );
}

/// `--debug` は実行が失敗しても統計と trace を表示する
#[test]
fn test_whitespace20_debug_reports_stats_on_failure() {
    let ws = compile_debug("func: __main() { __trace(5); __assert(0); }");
    let output = whitespace20(&["-", "--std-ext", "debug", "--debug"], &ws, None);
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("Total steps: "), "{}", stderr);
    assert!(stderr.contains("Traced values: {5: 1}"), "{}", stderr);
    assert!(
        stderr.contains("Runtime error: assertion failed"),
        "{}",
        stderr
    );
}

#[test]
fn test_whitespace20_step_limit_is_error() {
    let ws = compile_debug("func: __main() { let: i(0); while: 1 { i = i + 1; }; }");
    let output = whitespace20(&["-", "--max-steps", "100"], &ws, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("Execution limit exceeded (100 steps)"),
        "{}",
        stderr_of(&output)
    );
}

/// 終了時の stdout の flush の失敗は exit 1（従来は exit 0）
#[cfg(target_os = "linux")]
#[test]
fn test_whitespace20_flush_failure_is_error() {
    let ws = compile_debug("func: __main() { __puti(42); }");
    let output = whitespace20(&["-"], &ws, Some(dev_full()));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("I/O error"),
        "{}",
        stderr_of(&output)
    );
}

/// `--target json` は未実装のエラー（ライブラリの CompileTarget には無く、CLI の変換で拒否する）。
/// ソースを読む前に終了するので、stdin への書き込みが失敗しないよう空にする
#[test]
fn test_nospace20_json_target_is_unimplemented() {
    let output = nospace20(&["--target", "json"], "", None);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).contains("error: --target=json is not yet implemented"),
        "{}",
        stderr_of(&output)
    );
}

/// debug 拡張が有効なとき、__clog はプログラムの stdout ではなく stderr に書く
#[test]
fn test_whitespace20_clog_goes_to_stderr_with_debug_ext() {
    let ws = compile_debug("func: __main() { __clog(7); __puti(1); }");
    let output = whitespace20(&["-", "--std-ext", "debug"], &ws, None);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "1");
    assert!(
        stderr_of(&output).contains("__clog: 7"),
        "{}",
        stderr_of(&output)
    );
}

/// debug 拡張が無いとき、__clog は何も出力しない（stderr にも stdout にも出ない）
#[test]
fn test_whitespace20_clog_is_discarded_without_debug_ext() {
    let ws = compile_debug("func: __main() { __clog(7); __puti(1); }");
    let output = whitespace20(&["-"], &ws, None);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "1");
    assert!(
        !stderr_of(&output).contains("__clog"),
        "{}",
        stderr_of(&output)
    );
}
