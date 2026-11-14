#!/bin/bash
# Installs apt packages on a CI runner. The runners' Ubuntu mirror sometimes
# stalls mid-download, so a stalled connection is dropped and retried, and the
# whole install is tried again a few times before giving up.
set -euo pipefail

if [[ $# -eq 0 ]]; then
    echo "usage: $0 PACKAGE..." >&2
    exit 2
fi

options=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
for attempt in 1 2 3; do
    if sudo apt-get "${options[@]}" update &&
        sudo apt-get "${options[@]}" install -y "$@"; then
        exit 0
    fi
    echo "apt-get failed (attempt $attempt of 3)" >&2
    sleep $((attempt * 10))
done
exit 1
