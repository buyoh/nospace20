#!/bin/bash

set -eu
cd "$(dirname "$0")"

# release 
cargo build --release \
  --target wasm32-unknown-unknown --lib \
  --no-default-features --features wasm
wasm-pack build --target bundler --features wasm

# debug
cargo build \
  --target wasm32-unknown-unknown --lib \
  --no-default-features --features wasm
wasm-pack build --dev  --out-dir pkg-dev --target bundler --features wasm