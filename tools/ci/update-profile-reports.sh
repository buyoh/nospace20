#!/bin/bash

cd "$(dirname "$0")/../../"

VERSION_NUMVER=0
COMMIT_NUMBER=$(git rev-list --count HEAD)

PROFILE_FILENAME_PREFIX="profile-c${COMMIT_NUMBER}v${VERSION_NUMVER}"

cargo run --example ws_profiler -- --json \
  > "./profile-reports/${PROFILE_FILENAME_PREFIX}.json"
cargo run --example ws_profiler -- --json --std-ext alloc \
  > "./profile-reports/${PROFILE_FILENAME_PREFIX}-alloc.json"

if [ -f "./profile-reports/profile-latest.json" ]; then
  rm "./profile-reports/profile-latest.json"
fi
ln -s "${PROFILE_FILENAME_PREFIX}.json" "./profile-reports/profile-latest.json"
