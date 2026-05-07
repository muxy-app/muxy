#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

BOLD=$'\033[1m'
DIM=$'\033[2m'
RED=$'\033[31m'
GREEN=$'\033[32m'
YELLOW=$'\033[33m'
BLUE=$'\033[34m'
MAGENTA=$'\033[35m'
CYAN=$'\033[36m'
RESET=$'\033[0m'

usage() {
  cat <<EOF
Preview the release notes that would be generated for the next beta or stable
release. Calls the GitHub generate-notes API the same way the release workflows
do, so output matches the actual release.

Usage:
  scripts/preview-changelog.sh beta   [--raw]
  scripts/preview-changelog.sh stable [--from-beta vX.Y.Z-beta.N] [--version X.Y.Z] [--raw]

Beta mode:
  Version is computed from BETA_VERSION + git rev-list --count HEAD.
  Range starts at the most recent published beta tag.

Stable mode:
  --version defaults to BETA_VERSION (X.Y.Z).
  --from-beta defaults to the most recent beta tag and is used as the target
  ref (matching the release workflow's checkout). GitHub auto-picks the
  previous full release as the changelog start.

Options:
  --raw   Skip ANSI styling and print the markdown body as-is.
EOF
}

MODE="${1:-}"
if [[ -z "$MODE" || "$MODE" == "-h" || "$MODE" == "--help" ]]; then
  usage
  [[ -z "$MODE" ]] && exit 1 || exit 0
fi
shift

if [[ "$MODE" != "beta" && "$MODE" != "stable" ]]; then
  printf "%sError:%s mode must be 'beta' or 'stable' (got '%s')\n" "$RED" "$RESET" "$MODE" >&2
  exit 1
fi

FROM_BETA=""
VERSION_OVERRIDE=""
RAW=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --from-beta) FROM_BETA="${2:-}"; shift 2 ;;
    --version)   VERSION_OVERRIDE="${2:-}"; shift 2 ;;
    --raw)       RAW=1; shift ;;
    -h|--help)   usage; exit 0 ;;
    *) printf "%sError:%s unknown argument '%s'\n" "$RED" "$RESET" "$1" >&2; exit 1 ;;
  esac
done

if ! command -v gh >/dev/null 2>&1; then
  printf "%sError:%s 'gh' is required. Install with: brew install gh\n" "$RED" "$RESET" >&2
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  printf "%sError:%s 'gh' is not authenticated. Run: gh auth login\n" "$RED" "$RESET" >&2
  exit 1
fi

REPO="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"

BETA_BASE="$(tr -d '[:space:]' < BETA_VERSION)"
if ! [[ "$BETA_BASE" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf "%sError:%s BETA_VERSION must be X.Y.Z, got '%s'\n" "$RED" "$RESET" "$BETA_BASE" >&2
  exit 1
fi

latest_beta_tag() {
  gh release list --repo "$REPO" --limit 100 \
    --json tagName,isPrerelease \
    --jq '[.[] | select(.isPrerelease and (.tagName | contains("-beta.")))][0].tagName // ""'
}

latest_stable_tag() {
  gh release list --repo "$REPO" --limit 100 \
    --json tagName,isPrerelease,isDraft \
    --jq '[.[] | select((.isPrerelease | not) and (.isDraft | not))][0].tagName // ""'
}

hr() {
  local color="$1"
  printf "%s%s" "$BOLD" "$color"
  printf '━%.0s' {1..68}
  printf "%s\n" "$RESET"
}

print_header() {
  local channel="$1" version="$2" tag="$3" build="$4" target="$5" prev="$6" prev_label="$7"
  local color="$CYAN"
  [[ "$channel" == "Stable" ]] && color="$GREEN"

  printf "\n"
  hr "$color"
  printf "  %s%sMuxy Release Preview%s\n" "$BOLD" "$color" "$RESET"
  hr "$color"
  printf "  %sChannel:%s  %s\n" "$BOLD" "$RESET" "$channel"
  printf "  %sVersion:%s  %s\n" "$BOLD" "$RESET" "$version"
  printf "  %sTag:%s      %s\n" "$BOLD" "$RESET" "$tag"
  [[ -n "$build" ]] && printf "  %sBuild:%s    %s\n" "$BOLD" "$RESET" "$build"
  printf "  %sTarget:%s   %s\n" "$BOLD" "$RESET" "$target"
  if [[ -n "$prev" ]]; then
    printf "  %s%s:%s    %s\n" "$BOLD" "$prev_label" "$RESET" "$prev"
  else
    printf "  %s%s:%s    %s(none — full history)%s\n" "$BOLD" "$prev_label" "$RESET" "$DIM" "$RESET"
  fi
  hr "$color"
  printf "\n"
}

strip_noise() {
  awk '
    /^## New Contributors[[:space:]]*$/ { in_contrib = 1; next }
    in_contrib {
      if (/^## / || /^\*\*Full Changelog\*\*/) { in_contrib = 0 }
      else { next }
    }
    /^[*-] / {
      sub(/[[:space:]]+by[[:space:]]+@[A-Za-z0-9_-]+[[:space:]]+in[[:space:]]+https?:\/\/[^[:space:]]+[[:space:]]*$/, "")
      sub(/[[:space:]]+\(#[0-9]+\)[[:space:]]*$/, "")
    }
    { print }
  '
}

style_notes() {
  local notes
  notes="$(printf "%s\n" "$1" | strip_noise)"
  if [[ "$RAW" -eq 1 ]]; then
    printf "%s\n" "$notes"
    return
  fi
  printf "%s" "$notes" | awk \
    -v B="$BOLD" -v D="$DIM" -v C="$CYAN" -v M="$MAGENTA" -v G="$GREEN" -v R="$RESET" '
    /^## / {
      sub(/^## /, "")
      printf "%s%s%s%s\n\n", B, C, $0, R
      next
    }
    /^\*\*Full Changelog\*\*/ {
      gsub(/\*\*/, "")
      printf "\n%s%s%s\n", D, $0, R
      next
    }
    /^[*-] / {
      sub(/^[*-] /, "")
      printf "  %s•%s %s\n", G, R, $0
      next
    }
    /^### / {
      sub(/^### /, "")
      printf "%s%s%s%s\n", B, M, $0, R
      next
    }
    NF == 0 { print ""; next }
    { print }
  '
}

if [[ "$MODE" == "beta" ]]; then
  BUILD_NUMBER="$(git rev-list --count HEAD)"
  VERSION="${BETA_BASE}-beta.${BUILD_NUMBER}"
  TAG="v${VERSION}"
  PREV="$(latest_beta_tag)"
  HEAD_SHA="$(git rev-parse --short HEAD)"
  HEAD_BRANCH="$(git rev-parse --abbrev-ref HEAD)"

  ARGS=(--method POST "/repos/$REPO/releases/generate-notes"
        -f "tag_name=$TAG"
        -f "target_commitish=$(git rev-parse HEAD)")
  if [[ -n "$PREV" && "$PREV" != "$TAG" ]]; then
    ARGS+=(-f "previous_tag_name=$PREV")
  fi

  NOTES="$(gh api "${ARGS[@]}" --jq .body)"

  print_header "Beta" "$VERSION" "$TAG" "$BUILD_NUMBER" \
    "$HEAD_SHA ($HEAD_BRANCH)" "$PREV" "Since"
  style_notes "$NOTES"
  printf "\n"
  exit 0
fi

if [[ -z "$FROM_BETA" ]]; then
  FROM_BETA="$(latest_beta_tag)"
fi
if [[ -z "$FROM_BETA" ]]; then
  printf "%sError:%s no beta tag found and --from-beta not provided\n" "$RED" "$RESET" >&2
  exit 1
fi
if ! [[ "$FROM_BETA" =~ ^v[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+$ ]]; then
  printf "%sError:%s --from-beta must look like vX.Y.Z-beta.N (got '%s')\n" "$RED" "$RESET" "$FROM_BETA" >&2
  exit 1
fi

VERSION="${VERSION_OVERRIDE:-$BETA_BASE}"
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf "%sError:%s --version must be X.Y.Z (got '%s')\n" "$RED" "$RESET" "$VERSION" >&2
  exit 1
fi

TAG="v${VERSION}"
PREV_STABLE="$(latest_stable_tag)"

ARGS=(--method POST "/repos/$REPO/releases/generate-notes"
      -f "tag_name=$TAG"
      -f "target_commitish=$FROM_BETA")

NOTES="$(gh api "${ARGS[@]}" --jq .body)"

print_header "Stable" "$VERSION" "$TAG" "" "$FROM_BETA" "$PREV_STABLE" "Since"
style_notes "$NOTES"
printf "\n"
