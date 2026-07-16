#!/usr/bin/env zsh

set -euo pipefail

root="${0:A:h:h}"
binary="$root/.build/debug/Muxy"

args=()
for arg in "$@"; do
  case "$arg" in
    --hooks | -H)
      export FF_AI_HOOKS=1
      print -P "%F{yellow}AI hooks enabled%f — this dev build now owns the ~/.<provider> hook configs and stages hook scripts in Muxy Application Support. Relaunch the release app to restore its hook versions."
      ;;
    *)
      args+=("$arg")
      ;;
  esac
done

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

exec "$binary" "${args[@]}"
