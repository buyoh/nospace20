use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TestManifest {
    pub tests: Vec<TestCase>,
}

#[derive(Debug, Deserialize)]
pub struct TestCase {
    pub name: String,
    #[serde(rename = "type")]
    pub test_type: String,
    pub path: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub exclude_targets: Option<Vec<String>>,
    #[serde(default)]
    pub std_ext: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_std_ext: Option<Vec<String>>,
}

/// コメント行文字列を生成する。コメントがある場合は `// <comment>\n` を返す。
pub fn format_comment_line(comment: &Option<String>) -> String {
    if let Some(comment) = comment {
        format!("// {}\n", comment)
    } else {
        String::new()
    }
}

/// テストケースの exclude_targets から各ターゲットの有効/無効を判定する。
pub struct TargetFlags {
    pub has_interpreter: bool,
    pub has_interpreter_randomize: bool,
    pub has_whitespace: bool,
    pub has_whitespace_self: bool,
    pub has_whitespace_self_strict: bool,
    pub has_whitespace_self_randomize: bool,
    pub has_debug_ext: bool,
    pub has_alloc_ext: bool,
}

impl TargetFlags {
    pub fn from_test_case(test: &TestCase) -> Self {
        let empty_targets: Vec<String> = vec![];
        let exclude_targets = test.exclude_targets.as_ref().unwrap_or(&empty_targets);
        let has_debug_ext = test
            .exclude_std_ext
            .as_ref()
            .map(|exts| !exts.iter().any(|e| e == "debug"))
            .unwrap_or(true);
        let has_alloc_ext = test
            .std_ext
            .as_ref()
            .map(|exts| exts.iter().any(|e| e == "alloc"))
            .unwrap_or(false);
        Self {
            has_interpreter: !exclude_targets.iter().any(|t| t == "interpreter"),
            has_interpreter_randomize: !exclude_targets
                .iter()
                .any(|t| t == "interpreter-randomize"),
            has_whitespace: !exclude_targets.iter().any(|t| t == "whitespace"),
            has_whitespace_self: !exclude_targets.iter().any(|t| t == "whitespace-self"),
            has_whitespace_self_strict: !exclude_targets
                .iter()
                .any(|t| t == "whitespace-self-strict"),
            has_whitespace_self_randomize: !exclude_targets
                .iter()
                .any(|t| t == "whitespace-self-randomize"),
            has_debug_ext,
            has_alloc_ext,
        }
    }
}
