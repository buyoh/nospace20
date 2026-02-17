# Whitespace 重複ラベル定義のエラー検出

## 概要

Whitespace 仕様では、同一ラベル ID を持つ `Label` 命令の重複定義は不正であり、コンパイルエラーとすべきである。
現在の実装では重複を検出せず、後のラベル定義が前のものを上書きする動作となっている。

本タスクでは以下を実施する：

1. **テストケースの修正**: 重複ラベル定義テストを正常系からエラー系に変更
2. **実装調査**: 重複定義が発生している箇所の特定
3. **エラー検出の実装**: パース時に重複ラベルを検出してエラーを返す

## 現状の問題

### 問題箇所 1: `src/whitespace/interpreter.rs:241-248`

```rust
/// ラベル収集
fn collect_labels(instructions: &[Instruction]) -> HashMap<i64, usize> {
    let mut labels = HashMap::new();
    for (i, inst) in instructions.iter().enumerate() {
        if let Instruction::Label(id) = inst {
            labels.insert(id.to_ws_value(), i);
        }
    }
    labels
}
```

**問題点**:
- `HashMap::insert()` は既存のキーがあれば上書きする
- 重複ラベルがあっても何もエラーを出さない
- 後の定義で前の定義が上書きされ、前のラベル定義への参照が壊れる

### 問題箇所 2: パースエラー型に重複ラベルエラーがない

`src/whitespace/parser.rs:9-21` の `ParseError` 列挙型に、重複ラベルを示すエラーバリアントがない。

### テストケースの現状

- ファイル: `resources/tests_ws/passes/flow/duplicate_label_001.wsa`
- 現在の期待値: `{"type": "ws_io", "stdout": "13"}` (正常系)
- 本来あるべき: パースエラー or ランタイムエラー

## 実装計画

### Phase 1: エラー型の追加

**ファイル**: `src/whitespace/parser.rs`

`ParseError` に重複ラベルエラーを追加:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    // ... 既存のエラー ...
    
    /// 重複したラベル定義
    DuplicateLabel { label_id: i64, first_position: usize, second_position: usize },
}
```

### Phase 2: ラベル収集時の重複検出

**ファイル**: `src/whitespace/interpreter.rs`

`collect_labels()` を `Result` 型を返すように変更し、重複検出を実装:

```rust
/// ラベル収集（重複チェック付き）
fn collect_labels(instructions: &[Instruction]) -> Result<HashMap<i64, usize>, ParseError> {
    let mut labels = HashMap::new();
    for (i, inst) in instructions.iter().enumerate() {
        if let Instruction::Label(id) = inst {
            let label_value = id.to_ws_value();
            if let Some(&first_pos) = labels.get(&label_value) {
                return Err(ParseError::DuplicateLabel {
                    label_id: label_value,
                    first_position: first_pos,
                    second_position: i,
                });
            }
            labels.insert(label_value, i);
        }
    }
    Ok(labels)
}
```

### Phase 3: コンストラクタの修正

**ファイル**: `src/whitespace/interpreter.rs`

`from_source()` と `from_instructions()` を修正:

```rust
/// Whitespace テキストから VM を構築
pub fn from_source(source: &str) -> Result<Self, super::ParseError> {
    let instructions = super::parse(source)?;
    Self::from_instructions(instructions)
}

/// 命令列から VM を構築（重複ラベルチェック付き）
pub fn from_instructions(instructions: Vec<Instruction>) -> Result<Self, super::ParseError> {
    let labels = Self::collect_labels(&instructions)?;

    Ok(Self {
        instructions,
        labels,
        pc: 0,
        data_stack: Vec::new(),
        call_stack: Vec::new(),
        heap: HashMap::new(),
        stdin: Box::new(std::io::Cursor::new(Vec::new())),
        stdout: Box::new(Vec::<u8>::new()),
        total_steps: 0,
        traced: BTreeMap::new(),
        debug_ext: false,
        completed: false,
    })
}
```

### Phase 4: テストケースの修正

#### 4-1: テストタイプの変更

**ファイル**: `resources/tests_ws/test-manifest.yaml`

```yaml
- name: test_ws_errors_duplicate_label_001
  type: ws_parse_error  # ws_io から変更
  path: flow/duplicate_label_001
  comment: "Duplicate label definition (should be parse error)"
```

#### 4-2: チェック JSON の更新

**ファイル**: `resources/tests_ws/passes/flow/duplicate_label_001.check.json` → `resources/tests_ws/fails/parse/duplicate_label_001.check.json`

```json
{"type": "ws_parse_error", "error": "DuplicateLabel"}
```

#### 4-3: ファイル移動

```bash
mkdir -p resources/tests_ws/fails/parse/
mv resources/tests_ws/passes/flow/duplicate_label_001.wsa \
   resources/tests_ws/fails/parse/duplicate_label_001.wsa
mv resources/tests_ws/passes/flow/duplicate_label_001.check.json \
   resources/tests_ws/fails/parse/duplicate_label_001.check.json
```

### Phase 5: テストランナーの拡張

**ファイル**: `tests/whitespace_direct_test.rs`

`ws_parse_error` テストタイプのサポートを追加:

```rust
/// ws_parse_error テスト: パースエラー検証
fn test_ws_parse_error_base(test_name: &str) {
    let path_base = format!("resources/tests_ws/fails/parse/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base))
        .expect(&format!("Failed to read {}.wsa", test_name));
    let ws_code = decode_wsa(&wsa_content);

    let check: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(format!("{}.check.json", path_base))
            .expect(&format!("Failed to read {}.check.json", test_name)),
    ))
    .expect(&format!("Failed to parse check.json for {}", test_name));

    let expected_error = check
        .get("error")
        .and_then(|v| v.as_str())
        .expect(&format!("No 'error' field in check.json for {}", test_name));

    let result = WhitespaceVM::from_source(&ws_code);

    match result {
        Err(e) => {
            let error_name = match e {
                ParseError::DuplicateLabel { .. } => "DuplicateLabel",
                ParseError::InvalidImp { .. } => "InvalidImp",
                ParseError::InvalidCommand { .. } => "InvalidCommand",
                ParseError::UnexpectedEof { .. } => "UnexpectedEof",
                ParseError::InvalidNumber { .. } => "InvalidNumber",
                ParseError::InvalidLabel { .. } => "InvalidLabel",
            };
            assert_eq!(
                expected_error, error_name,
                "Test {} error type mismatch",
                test_name
            );
        }
        Ok(_) => panic!("Test {} expected parse error but parsing succeeded", test_name),
    }
}
```

### Phase 6: ビルドスクリプトの更新

**ファイル**: `build.rs`

`ws_parse_error` テストタイプを生成対象に追加。

## 影響範囲

### 変更が必要なファイル

1. `src/whitespace/parser.rs` - ParseError に DuplicateLabel 追加
2. `src/whitespace/interpreter.rs` - collect_labels() の Result 化、from_instructions() の修正
3. `src/whitespace/mod.rs` - エクスポート確認
4. `resources/tests_ws/test-manifest.yaml` - テストエントリ修正
5. `resources/tests_ws/fails/parse/duplicate_label_001.{wsa,check.json}` - ファイル移動・内容修正
6. `tests/whitespace_direct_test.rs` - ws_parse_error テストランナー追加
7. `build.rs` - ws_parse_error サポート追加

### 既存コードへの影響

`WhitespaceVM::from_instructions()` の戻り値型が変わるため、以下の呼び出し元を確認：

```bash
grep -r "from_instructions" src/
```

主な呼び出し元:
- `src/compiler_ws/mod.rs` - コンパイラパイプライン
- テストコード

これらは `?` 演算子でエラーを伝播させるか、適切にハンドリングする必要がある。

## テスト計画

### 追加テストケース

1. **duplicate_label_001.wsa** (既存):
   - 同じラベル「ABC」を2回定義
   - 期待: `DuplicateLabel` エラー

2. **duplicate_label_002.wsa** (新規):
   - ラベル「0」を2回定義（数値ラベル）
   - 期待: `DuplicateLabel` エラー

3. **duplicate_label_003.wsa** (新規):
   - 3つの異なるラベルの後、最初のラベルを再定義
   - 期待: `DuplicateLabel` エラー

### 既存テストへの影響

全ての ws_io / ws_runtime_error テストが引き続き合格することを確認:

```bash
cargo test whitespace_direct_test
```

## 完了条件

- [x] 調査完了: 重複ラベル定義箇所を特定
- [ ] Phase 1-6 の実装完了
- [ ] 全テスト合格
- [ ] コミット完了
- [ ] ドキュメント移動（完了後 → `ai-docs/done-task/`）

## 参考資料

- Whitespace 仕様: `spec-whitespace.md`
- 過去の修正: `ai-docs/done-task/fix-ws-self-label-duplication.md` (compiler_ws のラベル ID 重複バグ)
- テスト仕様: `resources/tests_ws/README.md`
