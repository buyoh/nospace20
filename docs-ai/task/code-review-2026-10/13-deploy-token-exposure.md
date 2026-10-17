# デプロイスクリプトで書き込み権限トークンを外部コードに晒している

- 重要度: **medium**
- 状態: 未着手

## 進捗

- [ ] トークンのスコープ縮小
- [ ] 外部リポジトリの固定
- [ ] 死にコード整理

## 問題点

- `tools/ci/deploy-webstatic-to-git.sh:17-19,33,45,55-57`
- `.github/workflows/deploy-web.yml:8-9,24-27`

1. `GH_TOKEN`（`contents: write`）がステップ全体の環境変数として渡された状態で、別リポジトリ（webui）の `main` 先頭（コミット未固定）を clone し、その `tools/update-nospace20.sh` と `npm ci` / `npm run test` / `npm run build-vite` を実行している。npm 依存の install script もトークンを読める。
2. 19 行目でトークン入り URL を組み立てるが、33 行目で `git config --get remote.origin.url` に上書きされ使われていない（死にコード、意図不明）。

既存指摘（code-review-2026-04 の `||:`・action の pin）とは別問題。

## 違反している原則

- 最小権限の原則、サプライチェーンセキュリティ

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `GH_TOKEN` を push 直前のステップにのみ渡す（ビルドとデプロイのステップ分離）。`npm ci --ignore-scripts` を検討 | `deploy-webstatic-to-git.sh`、`deploy-web.yml` |
| B | webui の clone を tag または SHA に固定 | 同上。webui 更新時に SHA 更新が必要 |
| C | 19 行目の死んだ分岐を削除、または 33 行目を条件付きにして意図を明確化 | `deploy-webstatic-to-git.sh` |

推奨: A + C、可能なら B。
