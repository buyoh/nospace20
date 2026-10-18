#!/bin/bash

set -eu

# Usage: deploy-webstatic-to-git.sh [build|push|all]
#   build: webui を clone してビルドする（外部コードを実行するため GH_TOKEN を渡してはならない）
#   push : ビルド済みの dist を deploy ブランチへ push する（GH_TOKEN を使うのはここだけ）
#   all  : build -> push（ローカル実行用。既定）
# GIT_REPO_WEBUI_REF: webui の clone 対象（ブランチ/タグ）。再現性のためタグの指定を推奨。

# -------------------------------------
# Constants

GIT_REPO_WEBUI_URL="https://github.com/buyoh/nospace20-webui.git"
GIT_REPO_WEBUI_BRANCH="${GIT_REPO_WEBUI_REF:-main}"

GIT_REPO_NOSPACE20_DEPLOY_BRANCH="deploy-webui"

PHASE="${1:-all}"

SCRIPT_DIR="$(realpath "$(dirname "$0")")"
REPO_ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"

TMP_DIR="$REPO_ROOT_DIR/target/tmp_webui_deploy"

# -------------------------------------

do_build() {
  # 外部コード（webui の更新スクリプト・npm）を実行するため、トークンが環境にあれば拒否する
  if [ -n "${GH_TOKEN:-}" ]; then
    echo "Error: GH_TOKEN must not be set in the build phase" >&2
    exit 1
  fi

  rm -rf "$TMP_DIR"
  mkdir -p "$TMP_DIR"

  cd "$REPO_ROOT_DIR"
  local nospace_version
  nospace_version=$(cargo pkgid | cut -d# -f2)

  cd "$TMP_DIR"
  git clone --depth 1 --branch "$GIT_REPO_WEBUI_BRANCH" "$GIT_REPO_WEBUI_URL" webui
  cd webui

  local webui_version
  webui_version=$(git log -1 --format="%cd" --date=format:"%Y.%m.%d")

  bash tools/update-nospace20.sh "$REPO_ROOT_DIR"

  cat > .env.local <<EOL
NODE_ENV=production
VITE_APPLICATION_FLAVOR=wasm
VITE_BASE_PATH="/nospace20/"
VITE_NOSPACE_VERSION=$nospace_version
VITE_WEBUI_VERSION=$webui_version
EOL

  # install script から外部コードが走るため無効化する
  npm ci --ignore-scripts
  npm run test  ||: # Check regression with new wasm
  npm run build-vite

  if [ ! -d "dist" ]; then
    echo "Error: Vite build failed, dist/ directory not found"
    exit 1
  fi
}

do_push() {
  if [ ! -d "$TMP_DIR/webui/dist" ]; then
    echo "Error: build output not found, run the build phase first" >&2
    exit 1
  fi

  # トークンがあれば認証付き URL、なければ（ローカル実行）origin の URL を使う
  local repo_url
  if [ -n "${GH_TOKEN:-}" ]; then
    repo_url="https://x-access-token:${GH_TOKEN}@github.com/${GH_REPOSITORY}.git"
  else
    repo_url=$(git -C "$REPO_ROOT_DIR" config --get remote.origin.url)
  fi

  cd "$TMP_DIR"
  rm -rf deploy
  mkdir deploy
  cd deploy
  git init
  git remote add origin "$repo_url"
  git branch -m "$GIT_REPO_NOSPACE20_DEPLOY_BRANCH"

  cp -r "$TMP_DIR/webui/dist/"* .

  git add .
  git commit -m "Deploy webui with updated wasm"

  # Force push to deploy branch (overwrite history)
  git push -f origin "$GIT_REPO_NOSPACE20_DEPLOY_BRANCH"
}

case "$PHASE" in
  build) do_build ;;
  push) do_push ;;
  all) (unset GH_TOKEN; do_build); do_push ;;
  *) echo "Usage: $0 [build|push|all]" >&2; exit 1 ;;
esac
