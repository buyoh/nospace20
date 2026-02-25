#!/bin/bash

cd "$(dirname "$0")/../../"

VERSION_NUMVER=0
COMMIT_NUMBER=$(git rev-list --count HEAD)

PROFILE_FILENAME="profile-c${COMMIT_NUMBER}v${VERSION_NUMVER}.json"

if [ -f "./profile-reports/${PROFILE_FILENAME}" ]; then
  echo "Profile report already exists: ${PROFILE_FILENAME}"
  exit 0
fi

cargo run --example ws_profiler -- --json > "./profile-reports/${PROFILE_FILENAME}"
if [ -f "./profile-reports/profile-latest.json" ]; then
  rm "./profile-reports/profile-latest.json"
fi
ln -s "${PROFILE_FILENAME}" "./profile-reports/profile-latest.json"
