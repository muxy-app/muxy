#!/usr/bin/env bash
set -euo pipefail

BOLD="\033[1m"
DIM="\033[2m"
RED="\033[31m"
GREEN="\033[32m"
YELLOW="\033[33m"
RESET="\033[0m"

FIX=0
for arg in "$@"; do
  case "$arg" in
    --fix) FIX=1 ;;
  esac
done

PASS="✓"
FAIL="✗"

steps=()
statuses=()
errors=()
durations=()
total_start=$SECONDS

format_duration() {
  local secs=$1
  if [ "$secs" -ge 60 ]; then
    printf "%dm %ds" $((secs / 60)) $((secs % 60))
  else
    printf "%ds" "$secs"
  fi
}

run_step() {
  local name="$1"
  shift
  steps+=("$name")
  local step_start=$SECONDS

  local tmpfile
  tmpfile=$(mktemp)

  if "$@" > "$tmpfile" 2>&1; then
    local elapsed=$(( SECONDS - step_start ))
    local dur
    dur=$(format_duration $elapsed)
    durations+=("$dur")
    statuses+=("pass")
    errors+=("")
    printf "  ${GREEN}${PASS}${RESET} %s ${DIM}%s${RESET}\n" "$name" "$dur"
    rm -f "$tmpfile"
    return 0
  else
    local exit_code=$?
    local elapsed=$(( SECONDS - step_start ))
    local dur
    dur=$(format_duration $elapsed)
    durations+=("$dur")
    statuses+=("fail")
    errors+=("$(cat "$tmpfile")")
    printf "  ${RED}${FAIL}${RESET} %s ${DIM}%s${RESET}\n" "$name" "$dur"
    rm -f "$tmpfile"
    return "$exit_code"
  fi
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANDROID_DIR="$SCRIPT_DIR/../android"

if [ ! -d "$ANDROID_DIR" ]; then
  printf "${RED}${FAIL}${RESET} android/ directory not found at %s\n" "$ANDROID_DIR"
  exit 1
fi

cd "$ANDROID_DIR"

if [ ! -x "./gradlew" ]; then
  chmod +x ./gradlew
fi

if ! command -v java &>/dev/null; then
  printf "${RED}${FAIL}${RESET} Java not found. Install JDK 17.\n"
  exit 1
fi

JAVA_VERSION=$(java -version 2>&1 | awk -F[\".] '/version/ {print $2}')
if [ -z "$JAVA_VERSION" ] || [ "$JAVA_VERSION" -lt 17 ]; then
  printf "${YELLOW}!${RESET} Java 17+ recommended. Found: $(java -version 2>&1 | head -1)\n"
fi

printf "\n"
failed=0

if [ "$FIX" -eq 1 ]; then
  run_step "ktlintFormat (fix)" ./gradlew ktlintFormat --no-daemon || failed=1
fi

if [ "$failed" -eq 0 ]; then
  run_step "Detekt" ./gradlew detekt --no-daemon || failed=1
fi

if [ "$failed" -eq 0 ]; then
  run_step "ktlintCheck" ./gradlew ktlintCheck --no-daemon || failed=1
fi

if [ "$failed" -eq 0 ]; then
  run_step "Android lint" ./gradlew lint --no-daemon || failed=1
fi

if [ "$failed" -eq 0 ]; then
  run_step "Unit tests" ./gradlew test --no-daemon || failed=1
fi

if [ "$failed" -eq 0 ]; then
  run_step "Assemble debug" ./gradlew :app:assembleDebug --no-daemon || failed=1
fi

printf "\n"

total_dur=$(format_duration $(( SECONDS - total_start )))

if [ "$failed" -ne 0 ]; then
  printf "${RED}${BOLD}  Failed${RESET} ${DIM}in %s${RESET}\n\n" "$total_dur"
  for i in "${!steps[@]}"; do
    if [ "${statuses[$i]}" = "fail" ] && [ -n "${errors[$i]}" ]; then
      printf "${DIM}─── %s ───${RESET}\n" "${steps[$i]}"
      echo "${errors[$i]}" | tail -50
      printf "\n"
    fi
  done
  exit 1
else
  printf "${GREEN}${BOLD}  All Android checks passed${RESET} ${DIM}in %s${RESET}\n\n" "$total_dur"
fi
