#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$SCRIPT_DIR/wsc-install"

echo "Installing whitespacers to $INSTALL_DIR ..."
cargo install whitespacers --root "$INSTALL_DIR"

echo "Done. wsc is available at: $INSTALL_DIR/bin/wsc"
echo "Version:"
"$INSTALL_DIR/bin/wsc" --help || :
