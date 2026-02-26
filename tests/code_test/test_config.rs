use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IoTestCase {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub stdin_file: Option<String>,
    #[serde(default)]
    pub stdout: Option<String>,
    #[serde(default)]
    pub stdout_file: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum TestConfig {
    Success {
        #[serde(alias = "trace")]
        trace_hit_counts: Vec<i64>,
    },
    SuccessIo {
        // 後方互換性のため残す（cases が未定義の場合に使用）
        #[serde(default)]
        stdin: Option<String>,
        #[serde(default)]
        stdin_file: Option<String>,
        #[serde(default)]
        stdout: Option<String>,
        #[serde(default)]
        stdout_file: Option<String>,
        // 新規追加: 複数ケースのサポート
        #[serde(default)]
        cases: Option<Vec<IoTestCase>>,
    },
    ParseError {
        phase: String, // "tokenize" or "tree"
        #[serde(skip_serializing_if = "Option::is_none")]
        error_count: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
    CompileError {
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
    RuntimeError {
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
}

// 後方互換性のため、"trace" フィールドのみの場合は Success として扱う
impl TestConfig {
    pub fn from_legacy(value: &serde_json::Value) -> Option<Self> {
        if value.get("type").is_none()
            && (value.get("trace").is_some() || value.get("trace_hit_counts").is_some())
        {
            let trace_hit_counts = value
                .get("trace_hit_counts")
                .or_else(|| value.get("trace"))?
                .as_array()?
                .iter()
                .map(|e| e.as_i64().unwrap())
                .collect();
            Some(TestConfig::Success { trace_hit_counts })
        } else {
            None
        }
    }

    /// SuccessIo テストから IoTestCase のリストを取得
    /// 後方互換性のため、cases が未定義の場合は従来のフィールドから1ケースを作成
    pub fn get_io_test_cases(&self) -> Vec<IoTestCase> {
        match self {
            TestConfig::SuccessIo {
                stdin,
                stdin_file,
                stdout,
                stdout_file,
                cases,
            } => {
                if let Some(cases) = cases {
                    // 新形式: cases が定義されている
                    cases.clone()
                } else {
                    // 旧形式: cases が未定義の場合、従来のフィールドから1ケースを作成
                    vec![IoTestCase {
                        name: Some("default".to_string()),
                        stdin: stdin.clone(),
                        stdin_file: stdin_file.clone(),
                        stdout: stdout.clone(),
                        stdout_file: stdout_file.clone(),
                    }]
                }
            }
            _ => panic!("Not a SuccessIo test config"),
        }
    }
}

/// check.json ファイルを読み込んで TestConfig を返す
pub fn load_check_json(path_base: &str) -> TestConfig {
    let check_json_value: serde_json::Value = serde_json::from_reader(std::io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();

    // 後方互換性: "trace" フィールドのみの場合
    if let Some(legacy) = TestConfig::from_legacy(&check_json_value) {
        legacy
    } else {
        match serde_json::from_value(check_json_value.clone()) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to parse config: {:?}", e);
                eprintln!("JSON value: {:?}", check_json_value);
                panic!("Failed to parse test config");
            }
        }
    }
}
