# 旧テストの移行 - Phase 2 実装完了報告

## 日時
2026-01-31

## 作業内容

### Phase 1 完了内容（前回）
- Environmentの拡張（stdin/stdout対応）
- I/Oビルトイン関数の実装（__puti, __putc, __geti, __getc）
- lib.rsへのinterpret_func_with_io追加
- テストハーネスのSuccessIo対応
- spec.mdへの仕様追記

### Phase 2 完了内容（今回）
- レガシーテスト001-005, 007-008の移行完了（7テスト成功）
- テストファイルをresources/tests/passes/legacy/に配置
- 各テストに対応する.check.jsonファイルを作成（success_io形式）

### 問題と未解決事項

#### パーサーの制約により移行できなかったテスト
- **legacy_009, legacy_010**: `} else:{`および`else:if:`構文が未サポート
  - 既存の`control_flow/if_001`も同じ構文で失敗していることを確認
  - これは今回の修正による問題ではなく、元々のパーサーの制約
  - SKILLの指示通り「別の原因の場合はテストをFailのまま残す」方針に従い、パーサー修正は後回し

#### 未実装機能により移行できなかったテスト
- **legacy_006, 011-012**: グローバル変数が必要（未実装）
- **legacy_013以降**: 配列・ポインタが必要（未実装）

### テスト結果
```
cargo test test_legacy_ --test code_test
test result: FAILED. 7 passed; 2 failed; 0 ignored; 0 measured; 32 filtered out
```

- 成功: legacy_001, 002, 003, 004, 005, 007, 008（7テスト）
- 失敗: legacy_009, 010（パーサー制約によるもの、修正対象外）

### 構文の発見事項
- if文・while文の条件式は括弧あり・なしどちらも正しい：`if: cond {` も `if: (cond) {` も同じ式として評価される
- if文の後にはセミコロンが必須：`if: cond { ... };`
- `else:{`の間にスペースは不要（トークンとして`Else`と`Colon`が別々に認識される）
- 負の数リテラルは未実装（`0-2`のような式で代用可能だが、legacy_010では必要なかった）

### 名称の整理
- 旧仕様で「ポインタ」と呼ばれていた機能は、C言語のポインタと区別するため「参照 (reference)」と呼ぶことに統一した

### ファイル配置
- レガシーテストは`resources/tests/passes/legacy/`に配置（旧実装との互換性テストであることを明示）
- 新規I/Oテストは`resources/tests/passes/io/`に配置予定（Phase 3で実装）

## 次のステップ
Phase 3:
- 新しいI/Oテストケースの作成（resources/tests/passes/io/）
- パーサーの拡張（`else:if:`構文のサポート）を検討
- legacy_009, 010の再有効化

Phase 4以降:
- グローバル変数の実装（legacy_006, 011-012のため）
- 配列・参照の実装（legacy_013以降のため）

### ドキュメント整備（完了）
- tutorial.mdの構文を現仕様に合わせて修正
- spec.mdに以下の機能仕様を追加:
  - 複合代入演算子 (`+=`, `-=`, `*=`, `/=`, `%=`)
  - 剰余演算子 (`%`)
  - 参照・間接参照演算子 (`&`, `*`)
  - 配列の宣言と初期化
  - 変数の初期化構文
