# 複数の入出力テストケース対応

## 概要

現在のテストシステムでは、`success_io` テストにおいて標準入出力を1組しかテストできない。複数の入出力パターンをテストしたい場合、複数のテストファイル (.ns + .check.json) を作成する必要があり、冗長である。

本タスクでは、1つのテストケースに対して複数の入出力パターンを定義できるようにする。

## 現状の課題

### 現在の success_io テストの構造

```json
{
  "type": "success_io",
  "stdin": "42\n100\n",
  "stdout": "42100"
}
```

または

```json
{
  "type": "success_io",
  "stdin_file": "input.txt",
  "stdout_file": "expected_output.txt"
}
```

### 問題点

1. **1つのテストケースに1組の入出力しか定義できない**
   - 例: `geti` の動作を複数の入力値でテストしたい場合、`geti_001.ns`, `geti_002.ns`, ... と複数のファイルを作る必要がある
   - 同じコードを複数のファイルに重複して書くことになる

2. **テストケースの管理が煩雑**
   - test-manifest.yaml に多数のエントリが必要
   - ファイル数が増加し、ディレクトリが肥大化

3. **エッジケースのテストが不十分になりがち**
   - 複数パターンのテストを追加するコストが高いため、テストケースの網羅性が低下する

## 提案する改善案

### 新しい success_io テストフォーマット

#### 案1: cases 配列を導入（推奨）

複数の入出力ケースを `cases` 配列で定義する。後方互換性のため、`cases` が未定義の場合は従来の単一ケース形式として扱う。

```json
{
  "type": "success_io",
  "cases": [
    {
      "name": "basic_case",
      "stdin": "42\n",
      "stdout": "42"
    },
    {
      "name": "zero_case",
      "stdin": "0\n",
      "stdout": "0"
    },
    {
      "name": "negative_case",
      "stdin": "-100\n",
      "stdout": "-100"
    }
  ]
}
```

**特徴:**
- `cases` 配列内の各要素が1つのテストケースを表す
- 各ケースに `name` を付けることで、失敗時のエラーメッセージを明確化
- `stdin`, `stdout`, `stdin_file`, `stdout_file` を各ケースで指定可能
- 後方互換性: `cases` が未定義の場合は従来通り `stdin`/`stdout` を使用

**後方互換性の処理:**

従来の形式（`cases` なし）:
```json
{
  "type": "success_io",
  "stdin": "ABC",
  "stdout": "ABC"
}
```

は、内部的に以下のように解釈される:
```json
{
  "type": "success_io",
  "cases": [
    {
      "name": "default",
      "stdin": "ABC",
      "stdout": "ABC"
    }
  ]
}
```

#### 案2: 配列形式でケースを列挙（シンプル）

```json
{
  "type": "success_io",
  "test_cases": [
    ["42\n", "42"],
    ["0\n", "0"],
    ["-100\n", "-100"]
  ]
}
```

**特徴:**
- よりコンパクト
- ただし、ケース名を付けられない（デバッグ性が低い）
- stdin_file/stdout_file が使えない

**結論:** 案1を採用する（柔軟性とデバッグ性を重視）

## 実装計画

### Phase 1: データ構造の拡張

#### 1.1 TestConfig の拡張

`tests/code_test.rs` の `TestConfig::SuccessIo` を拡張する。

**変更前:**
```rust
SuccessIo {
    #[serde(default)]
    stdin: Option<String>,
    #[serde(default)]
    stdin_file: Option<String>,
    stdout: Option<String>,
    stdout_file: Option<String>,
},
```

**変更後:**
```rust
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
    // 新規追加
    #[serde(default)]
    cases: Option<Vec<IoTestCase>>,
},
```

新しい構造体 `IoTestCase` を定義:
```rust
#[derive(Debug, Deserialize, Serialize)]
struct IoTestCase {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    stdin: Option<String>,
    #[serde(default)]
    stdin_file: Option<String>,
    #[serde(default)]
    stdout: Option<String>,
    #[serde(default)]
    stdout_file: Option<String>,
}
```

#### 1.2 ケースの正規化処理

後方互換性を保つため、`SuccessIo` から統一的な `Vec<IoTestCase>` を取得するヘルパーメソッドを実装:

```rust
impl TestConfig {
    fn get_io_test_cases(&self, path_base: &str) -> Vec<IoTestCase> {
        match self {
            TestConfig::SuccessIo { stdin, stdin_file, stdout, stdout_file, cases } => {
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
```

### Phase 2: テスト実行ロジックの更新

#### 2.1 test_ok_coding_io_base の変更

現在の実装は1ケースのみの実行。これを複数ケースに対応させる。

**変更後のロジック:**
```rust
fn test_ok_coding_io_base(test_name: &str) -> Result {
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
    let test_cases = check_json.get_io_test_cases(&path_base);

    // パース（全ケース共通）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();

    // 各ケースを実行
    for (idx, case) in test_cases.iter().enumerate() {
        let case_name = case.name.as_ref()
            .map(|n| n.as_str())
            .unwrap_or(&format!("case_{}", idx));

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
            panic!("IoTestCase must specify stdout or stdout_file");
        };

        // 実行
        let (_, actual_stdout) = interpret_func_with_io(&a, "main", &stdin_content);

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'\nExpected: {:?}\nActual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }

    Ok(())
}
```

#### 2.2 test_whitespace_io_base の変更

同様に、Whitespace コンパイラのテストも複数ケースに対応させる。

**変更点:**
- `get_io_test_cases()` を使用してケース一覧を取得
- 各ケースごとに Whitespace コードを実行
- エラーメッセージにケース名を含める

### Phase 3: ドキュメント更新

#### 3.1 README.md の更新

`resources/tests/README.md` に複数ケースの使用方法を記載する。

**追加内容:**

```markdown
### 複数の入出力ケース

1つのテストに複数の入出力パターンを定義できます。

**例:** [passes/io/geti_multiple_cases.check.json](passes/io/geti_multiple_cases.check.json)

```json
{
  "type": "success_io",
  "cases": [
    {
      "name": "positive",
      "stdin": "42\n",
      "stdout": "42"
    },
    {
      "name": "zero",
      "stdin": "0\n",
      "stdout": "0"
    },
    {
      "name": "negative",
      "stdin": "-100\n",
      "stdout": "-100"
    }
  ]
}
```

**フィールド:**
- `cases`: テストケースの配列
  - `name`: (オプション) ケースの識別名（テスト失敗時のメッセージに使用）
  - `stdin`: (オプション) 標準入力として与えるデータ
  - `stdin_file`: (オプション) 標準入力をファイルから読み込む
  - `stdout`: (オプション) 期待する標準出力の内容
  - `stdout_file`: (オプション) 期待する標準出力をファイルから読み込む

**後方互換性:**
従来の形式（`cases` を使わない形式）も引き続きサポートされます。

```json
{
  "type": "success_io",
  "stdin": "ABC",
  "stdout": "ABC"
}
```
```

#### 3.2 SKILL ドキュメントの更新

`.github/skills/add-test-spec/SKILL.md` を更新し、複数ケースの記述方法を追加する。

### Phase 4: 既存テストケースのマイグレーション（オプション）

必要に応じて、類似した複数のテストケースを1つにまとめる。

**例:**
- `geti_basic_001.ns` (42 をテスト)
- `geti_basic_002.ns` (0 をテスト)
- `geti_basic_003.ns` (-100 をテスト)

を、1つの `geti_various_inputs.ns` + 複数ケース定義にまとめる。

**注意:** 既存テストが正常に動作していることを確認するため、このフェーズは慎重に進める。

## テスト計画

### 単体テスト

1. **後方互換性のテスト**
   - 既存の success_io テスト（単一ケース）がすべてパスすることを確認
   - `cases` が未定義の場合、従来通り動作することを確認

2. **複数ケースのテスト**
   - 新しい形式（cases 配列）で複数ケースを定義
   - すべてのケースがパスすることを確認
   - 1つでも失敗した場合、エラーメッセージにケース名が含まれることを確認

3. **エッジケース**
   - `cases` が空配列の場合の挙動
   - `stdin`/`stdout` の両方が未定義のケースのエラーハンドリング

### 統合テスト

1. **既存テストの実行**
   - `cargo test` で全テストがパスすることを確認

2. **新規テストケースの作成**
   - 複数ケースを持つ新しいテストケースを作成
   - interpreter と whitespace の両方でテスト

## マイルストーン

- [ ] Phase 1: データ構造の拡張
  - [ ] `IoTestCase` 構造体の定義
  - [ ] `TestConfig::SuccessIo` の拡張
  - [ ] `get_io_test_cases()` ヘルパーメソッドの実装
  - [ ] 単体テスト（構造体のデシリアライズ）

- [ ] Phase 2: テスト実行ロジックの更新
  - [ ] `test_ok_coding_io_base` の変更
  - [ ] `test_whitespace_io_base` の変更
  - [ ] 既存テストがすべてパスすることを確認

- [ ] Phase 3: ドキュメント更新
  - [ ] `resources/tests/README.md` の更新
  - [ ] `.github/skills/add-test-spec/SKILL.md` の更新

- [ ] Phase 4: 新規テストケースの作成（検証用）
  - [ ] 複数ケースを持つサンプルテストの作成
  - [ ] interpreter と whitespace の両方でテスト
  - [ ] エラーケースのテスト（失敗時のメッセージ確認）

- [ ] Phase 5: 既存テストケースのマイグレーション（オプション）
  - [ ] 類似テストケースの統合候補を特定
  - [ ] 段階的にマイグレーション

## 備考

- 後方互換性を最優先とする
- 既存のテストがすべてパスすることを各フェーズで確認
- エラーメッセージにはケース名を含め、デバッグしやすくする
- ファイル参照（stdin_file/stdout_file）も引き続きサポート

## 関連ファイル

- `tests/code_test.rs`: テスト実行ロジック
- `build.rs`: テストコード生成
- `resources/tests/test-manifest.yaml`: テスト定義
- `resources/tests/README.md`: テストケースのドキュメント
- `.github/skills/add-test-spec/SKILL.md`: テスト追加のガイド
