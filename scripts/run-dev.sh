#!/usr/bin/env zsh

set -euo pipefail

root="${0:A:h:h}"
binary="$root/.build/debug/Muxy"
hook_binary="$root/.build/debug/muxy-hook"

args=()
for arg in "$@"; do
  case "$arg" in
    --hooks | -H)
      export FF_AI_HOOKS=1
      print -P "%F{yellow}AI hooks enabled%f — this dev build now owns the ~/.<provider> hook configs and stages files in Muxy's hooks-dev directory. Relaunch the release app to restore its hooks."
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

needs_hook_build=false

if [[ ! -x "$hook_binary" ]]; then
  needs_hook_build=true
else
  for path in \
    "$root/Package.swift" \
    "$root/Package.resolved" \
    "$root/MuxyHookBridge"/**/*(.) \
    "$root/MuxyHookKit"/**/*(.) \
    "$root/MuxyShared"/**/*(.); do
    if [[ "$path" -nt "$hook_binary" ]]; then
      needs_hook_build=true
      break
    fi
  done
fi

if [[ "$needs_build" == true ]]; then
  swift build --product Muxy --skip-update
fi

if [[ "$needs_hook_build" == true ]]; then
  swift build --product muxy-hook --skip-update
fi

exec "$binary" "${args[@]}"
