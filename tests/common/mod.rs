// Test utilities for running Whitespace code
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

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

/// Whitespace コードを実行して結果を取得
pub fn run_whitespace(ws_code: &str, stdin_input: &str) -> Result<String, String> {
    let wsc_path =
        find_wsc().ok_or_else(|| "wsc not found. Run: ./tools/setup-wsc.sh".to_string())?;

    // 一時ファイルに Whitespace コードを書き出し
    let mut file =
        NamedTempFile::new().map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(ws_code.as_bytes())
        .map_err(|e| format!("Failed to write temp file: {}", e))?;

    // wsc で実行
    let mut child = Command::new(&wsc_path)
        .arg(file.path())
        .args(["--unchecked-heap"]) // heap は0クリアで開始とみなす
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn wsc: {}", e))?;

    // stdin に入力を送信
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_input.as_bytes());
    }

    let output = child
        .wait_with_output()
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
