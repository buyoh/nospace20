# コードレビュー 2026-04

ソースコード・テストコード・設定ファイルの全体レビュー結果。
既出の `done-task/code-design-review/` とは重複しない新規指摘のみ記載。

---

## 1. デプロイスクリプトがテスト失敗を無視してデプロイを継続

**重要度: high**

### 問題点

`tools/ci/deploy-webstatic-to-git.sh` の 56 行目:

```bash
npm run test  ||: # Check regression with new wasm
```

`||:` によりテスト失敗時も後続処理が続行され、壊れた WASM がそのままデプロイされる。`set -eu` がスクリプト冒頭にあるため、通常はエラーで停止するはずだが、`||:` で明示的にエラーを無視している。

### 違反するルール

- 一般原則: CI/CD パイプラインではテスト失敗時にデプロイを停止すべき
- 品質ゲート: テストが失敗した成果物を本番環境にデプロイしてはならない

### 解決策

**案 A: `||:` を削除してテスト失敗で停止させる**

```bash
npm run test
npm run build-vite
```

`set -eu` が効いているため、`||:` を削除するだけでテスト失敗時にスクリプトが停止する。

**案 B: テスト失敗を警告としつつ、確認を挟む**

テスト未整備な段階で強制停止が不都合であれば、GitHub Actions 側で `continue-on-error: true` + 後続ステップでの確認分岐とする方が明示的。

**影響範囲**: `tools/ci/deploy-webstatic-to-git.sh`、`.github/workflows/deploy-web.yml`

---

## 2. GitHub Actions ワークフローのサプライチェーンリスク

**重要度: high**

### 問題点

`.github/workflows/fmt.yml` で使用しているサードパーティアクションがブランチ名で参照されている:

```yaml
- uses: ad-m/github-push-action@master
```

`@master` はミュータブルな参照であり、アップストリームのリポジトリが侵害された場合、任意のコードが CI 環境で実行される（サプライチェーン攻撃）。このアクションには `contents: write` 権限が付与されている。

### 違反するルール

- OWASP: ソフトウェアサプライチェーンセキュリティ
- GitHub 推奨: サードパーティアクションは SHA ピンニングの使用を推奨
- 一般原則: 最小権限の原則

### 解決策

**案 A: SHA ピンニングに変更**

```yaml
- uses: ad-m/github-push-action@d91a481090679876dfc4178fef17f286781251df  # v0.8.0
```

**案 B: `github-script` または `git push` コマンドに置換**

```yaml
- name: Push changes
  run: git push
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

サードパーティアクション自体を排除し、組み込みコマンドで代替する。

**影響範囲**: `.github/workflows/fmt.yml`

---

## 3. NospaceVM の unsafe ブロックに個別 SAFETY コメントが不足

**重要度: medium**

### 問題点

`src/interpreter/vm/eval.rs` に 4 箇所、`src/interpreter/vm/exec.rs` に 6 箇所の `unsafe` ブロックがあるが、各ブロック直前に SAFETY コメントが記載されていない。

`vm/mod.rs` に全体的な安全性の根拠（NospaceVM がスコープを所有し、raw pointer は生存期間中有効）が記載されているが、Rust のコーディング慣習では各 `unsafe` ブロックの直前に `// SAFETY:` コメントを書くことが推奨される。

```rust
// 現状 (eval.rs:17)
let expr = unsafe { &*expr_ptr };

// 推奨
// SAFETY: expr_ptr は NospaceVM が所有する Scope 内のデータへのポインタ。
// VM の生存期間中は Scope が変更されないため、参照は有効。
let expr = unsafe { &*expr_ptr };
```

### 違反するルール

- Rust コーディング慣習: `unsafe` ブロックには `// SAFETY:` コメントが必須 (clippy::undocumented_unsafe_blocks)
- 保守性: 安全性の根拠が分散していると、コード変更時に不変条件の確認が困難

### 解決策

各 `unsafe` ブロックの直前に `// SAFETY:` コメントを追加する。`vm/mod.rs` の全体説明を参照する形でも可。

```rust
// SAFETY: See module-level safety documentation in vm/mod.rs.
// The pointer is derived from a Box<T> owned by NospaceVM and remains valid
// for the lifetime of the VM execution.
```

**影響範囲**: `src/interpreter/vm/eval.rs`（4箇所）、`src/interpreter/vm/exec.rs`（6箇所）

---

## 4. WASM API の入力サイズ制限なし

**重要度: medium**

### 問題点

`src/wasm_api/api.rs` および `src/wasm_api/nospace_vm.rs` で受け取るソースコードと stdin に対してサイズ制限が設けられていない。悪意のあるクライアントが巨大な入力を送信した場合、ブラウザタブの OOM (Out of Memory) やフリーズを引き起こす可能性がある。

対象箇所:
- `api.rs`: `compile_nospace(source: String, ...)` — ソースコードサイズ無制限
- `nospace_vm.rs`: `NospaceVmWasm::run(stdin: String, ...)` — stdin サイズ無制限

### 違反するルール

- OWASP: 入力バリデーション不足
- 一般原則: 境界値での入力検証

### 解決策

**案 A: ソースコードと stdin にサイズ上限を設ける**

```rust
const MAX_SOURCE_SIZE: usize = 1_000_000;  // 1MB
const MAX_STDIN_SIZE: usize = 1_000_000;   // 1MB

if source.len() > MAX_SOURCE_SIZE {
    return Err(/* size limit error */);
}
```

**案 B: フロントエンド側で制限**

WASM を呼び出す JavaScript 側で入力サイズをバリデーションする。ただし、WASM API を直接使うケースでは防御にならない。

**影響範囲**: `src/wasm_api/api.rs`、`src/wasm_api/nospace_vm.rs`、および呼び出し元のフロントエンド

---

## 5. copilot-instructions.md のディレクトリ構造記述の不一致

**重要度: low**

### 問題点

`.github/copilot-instructions.md` のディレクトリ構造セクションに `src/compiler/` が記載されていない。実際のリポジトリには `src/compiler/grayspace/`（空ディレクトリ）が存在する。

また、overview.md のアーキテクチャ記述で `compiler/` が「未実装」と記載されているが、`compiler_ws/` とは別の `compiler/grayspace/` が存在する適切な背景説明がない。

### 違反するルール

- copilot-instructions.md: ディレクトリ構造を正確に反映すべき
- 一般原則: ドキュメントとコードの一貫性

### 解決策

**案 A: 空ディレクトリを削除**

`src/compiler/grayspace/` が未使用であれば削除し、必要になった時点で再作成する。

**案 B: ディレクトリ構造の記述を更新**

`compiler/grayspace/` が開発予定のモジュールであれば、copilot-instructions.md に追記する。

**影響範囲**: `.github/copilot-instructions.md`、`docs-ai/architecture/overview.md`、`src/compiler/`

---

## 6. fmt.yml ワークフローの設計上の懸念

**重要度: low**

### 問題点

`.github/workflows/fmt.yml` は全 push に対して `cargo fmt` → `cargo test` → 自動コミット＆プッシュを行う。

1. **競合リスク**: 複数の push が同時に行われた場合、fmt ワークフローの push が競合する
2. **不要なビルド**: フォーマット変更がない場合でもフルビルド＋テストが実行される（`cargo build` → `cargo fmt` → `cargo test`）
3. **ワークフローの連鎖**: fmt.yml の push が test.yml をトリガーし、不要なワークフロー実行が発生する可能性がある（`GITHUB_TOKEN` での push はワークフローをトリガーしない設定がデフォルトだが、設定によっては発生しうる）

### 違反するルール

- 一般原則: CI ワークフローの冪等性と効率性
- copilot-instructions.md: 「同時に別の Agent が修正を行っているかもしれない」— 並行作業への配慮

### 解決策

**案 A: フォーマットチェックのみに変更**

```yaml
- name: Check fmt
  run: cargo fmt --check
```

フォーマット違反はエラーとして報告し、自動修正は行わない。開発者がローカルで `cargo fmt` を実行する運用に変更。

**案 B: 現状維持（低優先度）**

ワークフローの無限ループは `GITHUB_TOKEN` では発生しないため、実害は軽微。リソース消費のみの問題。

**影響範囲**: `.github/workflows/fmt.yml`
