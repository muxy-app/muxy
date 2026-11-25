#!/bin/bash
# Guest account; input must come from export-source.py. Keeps the old source tree.
set -euo pipefail
archive="${1:?Usage: import-source.sh archive.tar}"
test -f "$HOME/.local/share/muxy-vm/managed"
mkdir -p "$HOME/src"
stage="$(mktemp -d "$HOME/src/.muxy-import.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
tar -xmf "$archive" -C "$stage"
test -f "$stage/Cargo.toml"
test -f "$stage/Cargo.lock"
test -f "$stage/crates/muxy-app/Cargo.toml"
if [ -e "$HOME/src/muxy" ]; then
  # Extracted paths carry the import time. Unchanged files, and directories with the same entries,
  # keep the old time so Cargo reuses their build.
  (cd "$stage" && find . -type f -print0) | while IFS= read -r -d '' file; do
    if [ -f "$HOME/src/muxy/$file" ] && cmp -s "$HOME/src/muxy/$file" "$stage/$file"; then
      touch -r "$HOME/src/muxy/$file" "$stage/$file"
    fi
  done
  (cd "$stage" && find . -mindepth 1 -type d -print0) | while IFS= read -r -d '' dir; do
    if [ -d "$HOME/src/muxy/$dir" ] && [ "$(ls -A "$HOME/src/muxy/$dir")" = "$(ls -A "$stage/$dir")" ]; then
      touch -r "$HOME/src/muxy/$dir" "$stage/$dir"
    fi
  done
  previous="$(mktemp -d "$HOME/src/muxy-previous.XXXXXX")"
  rmdir "$previous"
  mv "$HOME/src/muxy" "$previous"
  printf 'Previous source: %s\n' "$previous"
fi
mv "$stage" "$HOME/src/muxy"
trap - EXIT
printf 'Source ready: %s\n' "$HOME/src/muxy"
