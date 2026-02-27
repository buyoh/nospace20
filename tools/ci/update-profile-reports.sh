#!/bin/bash

set -eu

cd "$(dirname "$0")/../../"

ARGS=$@

VERSION_NUMVER=0
COMMIT_NUMBER=$(git rev-list --count HEAD)

PROFILE_FILENAME_PREFIX="profile-c${COMMIT_NUMBER}v${VERSION_NUMVER}"

cargo run --example ws_profiler -- $ARGS --json \
  > "./profile-reports/${PROFILE_FILENAME_PREFIX}.json"
cargo run --example ws_profiler -- $ARGS --json --std-ext alloc \
  > "./profile-reports/${PROFILE_FILENAME_PREFIX}-alloc.json"

if [ -f "./profile-reports/profile-latest.json" ]; then
  rm "./profile-reports/profile-latest.json"
fi
ln -s "${PROFILE_FILENAME_PREFIX}.json" "./profile-reports/profile-latest.json"

# tools/profile-report.py を使って、最新の比較プロファイルレポートを作成。profile-latestはリンクなので含めない。

# 非allocの比較レポート生成
COMPARE_ARGS=()
for f in ./profile-reports/profile-c*v*.json; do
  # シンボリックリンクを除外
  if [ -L "$f" ]; then continue; fi
  # allocファイルを除外
  if [[ "$f" == *-alloc.json ]]; then continue; fi
  label=$(basename "$f" .json | sed 's/^profile-//')
  COMPARE_ARGS+=(--label "$label" "$f")
done

if [ ${#COMPARE_ARGS[@]} -gt 0 ]; then
  python3 tools/profile-report.py "${COMPARE_ARGS[@]}" \
    -o "./profile-reports/profile-compare.html"
fi

# allocの比較レポート生成
COMPARE_ALLOC_ARGS=()
for f in ./profile-reports/profile-c*v*-alloc.json; do
  # シンボリックリンクを除外
  if [ -L "$f" ]; then continue; fi
  label=$(basename "$f" .json | sed 's/^profile-//')
  COMPARE_ALLOC_ARGS+=(--label "$label" "$f")
done

if [ ${#COMPARE_ALLOC_ARGS[@]} -gt 0 ]; then
  python3 tools/profile-report.py "${COMPARE_ALLOC_ARGS[@]}" \
    -o "./profile-reports/profile-compare-alloc.html"
fi