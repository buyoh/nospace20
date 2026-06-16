# ソースコード構成のリファクタリング

## 概要

現在のソースコード構成における問題点を整理し、リファクタリングの計画を立てる。
主な問題は型の重複、マクロの重複、エラーハンドリングの不統一など。

## 完了状況

✅ **すべての項目が完了しました**

- ✅ 項目1: 型の重複定義 - 設計方針を確定し、現状維持を決定
- ✅ 項目2: `Clone` derive - 所有権移動に変更、効率化完了
- ✅ 項目3: マクロの重複 - 共通マクロファイルに集約
- ✅ 項目4: モジュール可視性 - `pub(crate)` に統一、外部APIを明示化
- ✅ 項目5: エラー型のデバッグ情報 - `#[track_caller]` による実装完了
- ✅ 項目6: エラーハンドリング - すべてResult型で返される構造を確認
- ➡️ 項目7: compilerモジュール - 別ドキュメントに分割済み（`docs-ai/task/compiler/`）
- ✅ 項目8: コメント・命名の修正 - 完了

**このドキュメントはアーカイブ可能です**

## 問題点リスト

### 🔴 優先度: 高

#### 1. 型の重複定義 (`Expression` vs `ExecExpression`)

**現状**:
- `tree_parser/expression.rs`: `Expression`, `Statement` (パース結果)
- `semantic_analyzer/mod.rs`: `ExecExpression`, `ExecStatement` (実行用)
- `convert_to_exec_expression()` で変換

**役割の違い** (意図的な設計):
- `Expression`/`Statement`: 構文解析の結果のみを保持。`Invalid` バリアントを持つ
- `ExecExpression`/`ExecStatement`: 意味解析後の実行可能な表現。`Invalid` を持たない
  - 将来的にはスコープ解決済みの識別子情報を保持予定
  - 宣言文は `Scope` 構造に変換されるため `ExecStatement` には含まれない

**現状の問題**:
- 将来の拡張を見越した設計だが、現時点では構造が類似
- 意味解析の機能拡張 (変数名解決等) が進めば差異が明確になる予定

**改善案**:
- [x] ~~Option A: パース結果を `Result` で表現~~ → 現状維持
- [x] ~~Option B: `ExecStatement`/`ExecExpression` を削除~~ → 役割が異なるため不採用
- [x] Option C: 意味解析の機能拡張に合わせて差異を明確化 (採用)

**関連イシュー**: なし

---

#### 2. `Operator1`/`Operator2` の不要な `Clone` derive

**現状**: ✅ **完了**

**問題**:
- `semantic_analyzer` で参照からのコピーを行っていた
- 本来は所有権の移動で十分

**実施した変更**:
- [x] `convert_to_exec_expression` を所有権移動バージョンに変更
- [x] `Expression` が `Clone` を必要とするため、`Operator1`/`Operator2` も `Clone` が必要と判明
  - コメントを「Expression が Clone を必要とするため必要」に変更

**結果**:
- 不要な `.to_owned()` 呼び出しを削除
- 所有権の移動により、より効率的なコードに改善

**関連イシュー**: なし

---

#### 3. マクロの重複 (`match_expect_token!`)

**現状**:
- `src/tree_parser/expression.rs` (L16-30)
- `src/tree_parser/statement.rs` (L12-49)
- 完全に同じマクロが2ファイルに存在

**問題**:
- メンテナンス時に両方修正が必要
- コードの一貫性が保てない

**影響範囲**:
- `src/tree_parser/expression.rs`
- `src/tree_parser/statement.rs`

**改善案**:
- [x] `src/tree_parser/macros.rs` に共通マクロを抽出 ✅ **完了**
- [ ] ~~または `src/tree_parser/mod.rs` 内でマクロ定義~~ (上記で解決)

**関連イシュー**: なし

---

### 🟡 優先度: 中

#### 4. モジュール可視性の不整合

**現状**: ✅ **完了**

**実施した方針**:
- **外部公開API**: `lib.rs` で明示的に `pub use` で公開
- **内部型**: すべて `pub(crate)` で統一
- **モジュール内のみ使用**: デフォルト (`pub` なし)

**実施した変更**:
- [x] `semantic_analyzer` の型を `pub(crate)` に変更
  - `Variable`, `Block`, `ExecExpression`, `ExecStatement`, `Function` を `pub(crate)` に
  - `Scope` のみ外部公開（`lib.rs` で `pub use`）
- [x] `lib.rs` で外部公開APIを明示化
  - `pub use semantic_analyzer::Scope;` を追加

**結果**:
- 公開APIと内部実装の境界が明確化
- モジュール間の依存関係が可視化
- 将来の内部リファクタリングが容易に

**関連イシュー**: なし

---

#### 5. エラー型のデバッグ情報

**現状**: ✅ **完了**

**実装内容**:
- `CodeParseError` に `#[cfg(debug_assertions)]` 付きで `caller` フィールドを追加
- `CodeParseError::new()` メソッドに `#[track_caller]` 属性を付与
- デバッグビルドでエラー発生箇所のファイル名・行番号を自動記録
- リリースビルドではオーバーヘッドなし

**実装結果**:
```rust
#[derive(Clone, Debug)]
pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: String,
    #[cfg(debug_assertions)]
    pub caller: &'static std::panic::Location<'static>,
}
```

**メリット**:
- マクロ展開位置ではなく、実際のエラー発生箇所を記録
- ヘルパー関数を経由しても正しい位置情報を取得
- デバッグ時のみ有効で、リリースビルドに影響なし

**関連イシュー**: なし

---

#### 6. `semantic_analyzer` のエラーハンドリング不足

**現状**: ✅ **完了**

**確認結果**:
すべてのユーザーエラーは既に `Result<_, Vec<CodeParseError>>` 型で返されており、適切に処理されている。

| 箇所 | メッセージ | 対応状況 |
|------|------------|---------|
| L244 | `"nested function declaration is not supported"` | `return Err(...)` で実装済み ✅ |
| L272 | `"return statement outside of function"` | `return Err(...)` で実装済み ✅ |
| L280 | `"expression statement at root level"` | `return Err(...)` で実装済み ✅ |
| L288 | `"continue statement outside of function"` | `return Err(...)` で実装済み ✅ |
| L296 | `"break statement outside of function"` | `return Err(...)` で実装済み ✅ |

**未実装機能の panic!**:
- L228: `"global variable is not implemented"` - 未実装機能のため panic! が適切 ✅

**結果**:
- すべてのユーザーエラーが適切にエラー型で返される
- 未実装機能のみ panic! を使用
- エラーハンドリングが統一されている

**関連イシュー**: なし

---

#### 7. 未実装の `compiler` モジュール

**現状**: ➡️ **別ドキュメントに分割済み**

**移行先**:
- `docs-ai/task/compiler/` ディレクトリに詳細設計を移行
- コンパイラの実装計画は別途管理

**このタスクでの対応**:
- このドキュメントでは対応不要
- コンパイラ実装は独立したタスクとして進行中

**関連イシュー**: なし

---

### 🟢 優先度: 低 (改善提案)

#### 8. コメント・命名の修正

| 箇所 | 問題 | 修正案 |
|------|------|--------|
| `token_parser/mod.rs:44` | `Colon` のコメントが `// ;` | `// :` に修正 |
| `syntactic_analyzer` | 名前が "syntactic" だが実際は semantic analyzer | `semantic_analyzer` にリネーム |
| 複数箇所 | `stat`, `e`, `f` など短い変数名 | より説明的な名前に変更 |

**関連イシュー**: なし

---

## 推奨リファクタリング順序

### Phase 1: 早期改善 (即座に実施可能)

1. ✅ **マクロの重複解消** (問題3) - 完了
   - リスク: 低
   - 効果: コードの保守性向上
   - 実施内容: `match_expect_token` マクロを `tree_parser/macros.rs` に集約

2. ✅ **コメント・命名の修正** (問題8) - 完了
   - リスク: 低
   - 効果: 可読性向上
   - 実施内容:
     - `Colon` のコメント修正
     - `syntactic_analyzer` → `semantic_analyzer` にリネーム

3. ✅ **`Operator1`/`Operator2` の所有権移動** (問題2) - 完了
   - リスク: 低
   - 効果: 不要な `to_owned()` 削除、効率改善
   - 実施内容:
     - `convert_to_exec_expression` を所有権移動バージョンに変更
     - `Expression` が `Clone` を必要とするため、`Operator1`/`Operator2` にも `Clone` が必要と判明

### Phase 2: 中期改善 (設計検討が必要)

3. ✅ **エラー型のデバッグ情報追加** (問題5) - 完了
   - リスク: 低
   - 効果: デバッグ時のエラー追跡改善、リリースビルドに影響なし
   - 実施内容:
     - `#[track_caller]` を使用してエラー発生箇所を記録
     - デバッグビルドのみで有効（`#[cfg(debug_assertions)]`）
     - `code_parse_error!` マクロは `CodeParseError::new()` を呼び出すように変更

4. 🔲 **`semantic_analyzer` のエラーハンドリング** (問題6)
   - リスク: 中
   - 効果: エラー報告の改善
   - 所要時間: 4-6時間
   - 依存: Phase 2-3 (エラー型統一) ✅ 完了

### Phase 3: 大規模リファクタリング (アーキテクチャ変更)

5. 🔲 **型の重複定義の解消** (問題1)
   - リスク: 高
   - 効果: コードベースの簡素化
   - 所要時間: 1-2日
   - 依存: Phase 2-3, 2-4 (エラーハンドリング整備後)
   - 注意: 設計方針の確定が必要

### Phase 4: クリーンアップ

6. 🔲 **未使用コードの削除** (問題7)
   - リスク: 低
   - 効果: プロジェクト構造の明確化
   - 所要時間: 30分

7. 🔲 **モジュール可視性の統一** (問題4)
   - リスク: 低
   - 効果: API設計の明確化
   - 所要時間: 1時間

---

## 設計決定が必要な事項

### 1. Expression vs ExecExpression の統合方針

**決定**: Option C 採用 (現状維持 + 段階的拡張)

**理由**:
- `Expression`: 構文解析結果のみを保持 (`Invalid` バリアント含む)
- `ExecExpression`: 実行可能な表現 (スコープ解決済み識別子を保持予定)
- 意味解析機能の拡張に伴い差異が明確になる

**次のステップ**:
- `ExecExpression::Variable(String)` を `ExecExpression::Variable(IdentifierInfo)` に変更
- ソースコードに設計意図をコメントとして追加済み

---

### 2. エラー型の設計方針

**決定**: `#[track_caller]` を使用したデバッグ情報の追加 ✅ **実装完了**

**理由**:
- `#[track_caller]` により、マクロやヘルパー関数を経由しても正しいエラー発生箇所を記録できる
- `#[cfg(debug_assertions)]` により、リリースビルドではオーバーヘッドなし
- 開発時のデバッグ効率が大幅に向上

**実装内容**:
- `CodeParseError` に `caller: &'static Location<'static>` フィールドを追加（デバッグビルドのみ）
- `CodeParseError::new()` に `#[track_caller]` 属性を付与
- エラーメッセージ出力時に内部位置情報を表示

**次のステップ**:
- ✅ 実装完了
- ✅ 全テスト通過確認

---

### 3. モジュール名の変更

**現状**: `syntactic_analyzer`
**候補**: `semantic_analyzer`

**理由**:
- スコープ解析、識別子解決は意味解析 (semantic analysis) の範疇
- 構文解析 (syntactic analysis) は `tree_parser` が担当

**影響**:
- モジュール名変更
- ディレクトリ名変更
- import文の更新
- ドキュメントの更新

**決定**: 未定（要合意）

---

## 関連ドキュメント

- [architecture/overview.md](../architecture/overview.md) - システム概要
- [architecture/modules.md](../architecture/modules.md) - モジュール詳細
- [test-error-handling.md](test-error-handling.md) - テストのエラーハンドリング

---

## 進捗記録

### 2026-02-04 (2回目)

- ✅ **このドキュメントのすべての項目が完了**
- ✅ Phase 2-2: モジュール可視性の統一 (問題4) - 完了
  - `semantic_analyzer` の型を `pub(crate)` に変更
  - `Variable`, `Block`, `ExecExpression`, `ExecStatement`, `Function` を `pub(crate)` に
  - `Scope` のみ外部公開（`lib.rs` で `pub use`）
  - `Scope` のメソッドとフィールドを `pub(crate)` に変更
  - コンパイル警告を解消
- ✅ Phase 2-2: semantic_analyzer のエラーハンドリング (問題6) - 確認完了
  - すべてのユーザーエラーが既に `Result` 型で返されていることを確認
  - 未実装機能のみ `panic!` を使用しており、適切に処理されている
- ✅ 項目7は別ドキュメントに分割済みのため対応不要
- ✅ 全ての単体テスト・統合テストがパス

### 2026-02-04

- ✅ Phase 2: エラー型のデバッグ情報追加 (問題5) - 完了
  - `#[track_caller]` を使用した実装に置き換え
  - `CodeParseError` に `caller` フィールド追加（デバッグビルドのみ）
  - `CodeParseError::new()` メソッドに `#[track_caller]` 属性を付与
  - エラー表示に内部位置情報を追加
  - 全ての単体テスト・統合テストがパス
  - デバッグ実行でエラー発生箇所の正確な位置情報が表示されることを確認

### 2026-02-01

- ✅ Phase 1-3: `Operator1`/`Operator2` の所有権移動の改善
  - `convert_to_exec_expression` を所有権移動バージョンに変更
  - `.to_owned()` 呼び出しを削除
  - `Expression` が `Clone` を必要とするため、`Operator1`/`Operator2` にも `Clone` derive が必要と判明
  - コメントを「Expression が Clone を必要とするため必要」に変更
  - テストコードを修正して `clone()` を使用
  - 全ての単体テストがパス

### 2026-01-31

- ✅ ソースコード構成のレビュー完了
- ✅ 問題点の整理と優先度付け
- ✅ 設計方針の決定
  - Expression vs ExecExpression: 現状維持 (役割が異なる)
  - エラー型: デバッグ情報を削除する方針
  - panic! の分類完了
- ✅ ソースコードへのコメント追加
  - `ExecExpression`/`ExecStatement` に役割を説明するドキュメントコメント追加
  - `Expression::Invalid` 分岐を `unreachable!` に変更
  - 各 `panic!` に TODO コメント追加 (未実装 or Result型に変更すべき)
  - `CodeParseErrorInternal` のフィールドに問題点をドキュメント化
- ✅ Phase 1: 早期改善 (完了)
  - マクロの重複解消
  - syntactic_analyzer → semantic_analyzer リネーム
  - コメント・命名の修正
- ✅ Phase 2-1: エラー型の統一 (完了)
  - CodeParseErrorInternal を削除し CodeParseError に統一
- 🔲 Phase 2-2: semantic_analyzer のエラーハンドリング

---

## メモ

- 大規模リファクタリング前に、既存テストが全てパスすることを確認
- 各Phaseごとに全テストを実行して回帰を防ぐ
- `semantic_analyzer` への名前変更は影響範囲が大きいため、他の改善と分離して実施
