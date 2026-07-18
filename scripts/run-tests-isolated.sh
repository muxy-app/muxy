#!/usr/bin/env bash
set -euo pipefail

isolated_home=$(mktemp -d "${TMPDIR:-/tmp}/muxy-tests.XXXXXX")

cleanup() {
  rm -rf -- "$isolated_home"
}

trap cleanup EXIT INT TERM

export CFFIXED_USER_HOME="$isolated_home"
export MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$isolated_home/Library/Application Support/Muxy"
"$@"
