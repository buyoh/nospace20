#!/bin/bash

set -eu

# -------------------------------------
# Constants

GIT_REPO_WEBUI_URL="https://github.com/buyoh/nospace20-webui.git"
GIT_REPO_WEBUI_BRANCH="main"

GIT_REPO_NOSPACE20_DEPLOY_BRANCH="deploy-webui"

WEBUI_WASM_DEST_DIR="src/web/libs/nospace20"

# -----------------

SCRIPT_DIR="$(realpath "$(dirname "$0")")"
REPO_ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"

TMP_DIR="$REPO_ROOT_DIR/target/tmp_webui_deploy"

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

# -------------------------------------

cd "$REPO_ROOT_DIR"
GIT_REPO_NOSPACE20_URL=$(git config --get remote.origin.url)

# -------------------------------------

cd "$REPO_ROOT_DIR"
NO_DEBUG=true bash "build-wasm.sh"

if [ ! -f "$REPO_ROOT_DIR/pkg/"*.wasm ]; then
  echo "Error: wasm build failed, pkg/wasm_bg.wasm not found"
  exit 1
fi

# -------------------------------------

cd "$TMP_DIR"
git clone --depth 1 --branch "$GIT_REPO_WEBUI_BRANCH" "$GIT_REPO_WEBUI_URL" webui
cd webui

# Update webui with new wasm files
cp "$REPO_ROOT_DIR/pkg/"*.wasm "$REPO_ROOT_DIR/pkg/"*.js "$REPO_ROOT_DIR/pkg/"*.ts \
  "$WEBUI_WASM_DEST_DIR/"

cat > .local.env <<EOL
NODE_ENV=production
VITE_APPLICATION_FLAVOR=wasm
VITE_BASE_PATH="/nospace20/editor/"
EOL

npm ci
npm run test  ||: # Check regression with new wasm
npm run build-vite

if [ ! -d "dist" ]; then
  echo "Error: Vite build failed, dist/ directory not found"
  exit 1
fi

# -------------------------------------

cd "$TMP_DIR"

mkdir deploy
cd deploy
git init
git remote add origin "$GIT_REPO_NOSPACE20_URL"
git branch -m "$GIT_REPO_NOSPACE20_DEPLOY_BRANCH"

cp -r "$TMP_DIR/webui/dist/"* .

git add .
git commit -m "Deploy webui with updated wasm"

# Force push to deploy branch (overwrite history)
git push -f origin "$GIT_REPO_NOSPACE20_DEPLOY_BRANCH"




