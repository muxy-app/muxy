#!/usr/bin/env zsh

set -euo pipefail

root="${0:A:h:h}"
binary="$root/.build/debug/Muxy"

needs_build=false

if [[ ! -x "$binary" ]]; then
  needs_build=true
else
  for path in \
    "$root/Package.swift" \
    "$root/Package.resolved" \
    "$root/Muxy"/**/*(.) \
    "$root/MuxyShared"/**/*(.) \
    "$root/MuxyServer"/**/*(.) \
    "$root/GhosttyKit"/**/*(.); do
    if [[ "$path" -nt "$binary" ]]; then
      needs_build=true
      break
    fi
  done
fi

if [[ "$needs_build" == true ]]; then
  swift build --product Muxy --skip-update
fi

exec "$binary" "$@"
