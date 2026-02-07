````markdown
# 結合テスト設計

## 概要

ユニットテストでは各モジュールを独立してテストするが、手動構築が煩雑になる場合がある。
このタスクでは、複数モジュールを連携させた「結合テスト」の設計・計画を行う。

## 背景

- ユニットテストでは token_parser / tree_parser / semantic_analyzer を独立してテスト
- 入力を手動構築するため、複雑なケースでは可読性・保守性が低下する
- コード文字列から意味解析までの一連の流れをテストする結合テストが必要
- interpreter は実行環境であり、解析フェーズとは異なるため、このタスクではスコープ外とする

## 結合テストの位置づけ

| テスト種別 | 対象 | 入力形式 | 特徴 |
|-----------|------|---------|------|
| ユニットテスト | 単一モジュール | 手動構築 | 高速、依存なし、詳細な検証 |
| 結合テスト | 解析パイプライン | コード文字列 | パーサー/意味解析の連携確認 |
| largeテスト | 全パイプライン | .ns ファイル | E2E、実行確認 |

## 結合テストの種類

### 1. token_parser + tree_parser

コード文字列からAST構築までをテスト。

```rust
fn test_parse_from_str(code: &str) -> Vec<Statement> {
    let tokens = token_parser::parse_to_tokens(code).unwrap();
    tree_parser::parse_to_tree(&tokens).unwrap()
}
```

### 2. token_parser + tree_parser + semantic_analyzer

コード文字列からScope構築までをテスト。意味解析の最終段階まで確認する。

```rust
fn test_analyze_from_str(code: &str) -> Scope {
    let tokens = token_parser::parse_to_tokens(code).unwrap();
    let statements = tree_parser::parse_to_tree(&tokens).unwrap();
    semantic_analyzer::analyze(&statements).unwrap()
}
```

## テストケース管理

### 外部ファイル形式の検討

複雑なテストケースは外部ファイル（JSON/YAML）で管理することで可読性を向上させる。

```yaml
# resources/tests/integration/parser/test_cases.yaml
- name: simple_function
  input: |
    fn main() {
      return 42
    }
  expected_ast:
    - type: FunctionDeclaration
      name: main
      args: []
      body:
        - type: Return
          value: { type: Number, value: 42 }
```

### ディレクトリ構造案

```
resources/
├── tests/
│   ├── passes/          # 既存の large テスト
│   ├── fails/           # 既存のエラーテスト
│   └── integration/     # 結合テスト用データ
│       ├── parser/      # token_parser + tree_parser
│       └── analyzer/    # + semantic_analyzer
└── unit-tests/          # ユニットテスト用データ
    ├── tree_parser/
    └── semantic_analyzer/
```

## タスク

### Phase 1: 設計

- [ ] **D1-1**: 結合テストのヘルパー関数設計
- [ ] **D1-2**: 外部ファイル形式の決定（JSON vs YAML）
- [ ] **D1-3**: ディレクトリ構造の決定

### Phase 2: 実装

- [ ] **I2-1**: 結合テスト用ヘルパー関数の実装
- [ ] **I2-2**: 外部ファイル読み込み機構の実装
- [ ] **I2-3**: テストケースの作成

## 優先度

**低** - まずはユニットテストを整備してから実施

## 関連タスク

- [unit-test-tree-parser.md](unit-test-tree-parser.md)
- [unit-test-semantic-analyzer.md](unit-test-semantic-analyzer.md)

````
