#!/bin/bash

set -eu

# Usage: deploy-webstatic-to-git.sh [--workflow]
#   webui を clone してビルドし、dist を deploy ブランチへ commit して push する。
#   --workflow: CI 用。commit までで push しない（push は workflow 側の ad-m/github-push-action が行う）。
#               ローカル手動実行では指定しない。
# このスクリプトは認証情報を扱わない（外部コードを実行するため、トークンを環境に渡してはならない）。
# GIT_REPO_WEBUI_REF: webui の clone 対象（ブランチ/タグ）。再現性のためタグの指定を推奨。

# -------------------------------------
# Constants

GIT_REPO_WEBUI_URL="https://github.com/buyoh/nospace20-webui.git"
GIT_REPO_WEBUI_BRANCH="${GIT_REPO_WEBUI_REF:-main}"

GIT_REPO_NOSPACE20_DEPLOY_BRANCH="deploy-webui"

SKIP_PUSH=0
case "${1:-}" in
  "") ;;
  --workflow) SKIP_PUSH=1 ;;
  *) echo "Usage: $0 [--workflow]" >&2; exit 1 ;;
esac

SCRIPT_DIR="$(realpath "$(dirname "$0")")"
REPO_ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"

TMP_DIR="$REPO_ROOT_DIR/target/tmp_webui_deploy"

# -------------------------------------

do_build() {
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

  cat > env.local.js <<EOL
module.exports = {
  NODE_ENV: 'production',
  VITE_APPLICATION_FLAVOR: 'wasm',
  VITE_BASE_PATH: '/nospace20/',
  VITE_NOSPACE_VERSION: '$nospace_version',
  VITE_WEBUI_VERSION: '$webui_version'
};
EOL

  bash tools/build-webui.sh

  if [ ! -d "dist" ]; then
    echo "Error: Vite build failed, dist/ directory not found"
    exit 1
  fi
}

do_commit_and_push() {
  cd "$TMP_DIR"
  rm -rf deploy
  mkdir deploy
  cd deploy
  git init
  if [ "$SKIP_PUSH" -eq 0 ]; then
    git remote add origin "$(git -C "$REPO_ROOT_DIR" config --get remote.origin.url)"
  fi
  git branch -m "$GIT_REPO_NOSPACE20_DEPLOY_BRANCH"

  cp -r "$TMP_DIR/webui/dist/"* .

  git add .
  git commit -m "Deploy webui with updated wasm"

  if [ "$SKIP_PUSH" -eq 1 ]; then
    echo "--workflow: skip push (pushed by the CI action)"
    return 0
  fi

  # Force push to deploy branch (overwrite history)
  git push -f origin "$GIT_REPO_NOSPACE20_DEPLOY_BRANCH"
}

do_build
do_commit_and_push
