#!/usr/bin/env bash
set -euo pipefail

awk '
  /^## New Contributors[[:space:]]*$/ { in_contrib = 1; next }
  in_contrib {
    if (/^## / || /^\*\*Full Changelog\*\*/) { in_contrib = 0 }
    else { next }
  }
  /^\*\*Full Changelog\*\*/ { next }
  /^[*-] / {
    sub(/[[:space:]]+by[[:space:]]+@[A-Za-z0-9_-]+[[:space:]]+in[[:space:]]+https?:\/\/[^[:space:]]+[[:space:]]*$/, "")
    sub(/[[:space:]]+\(#[0-9]+\)[[:space:]]*$/, "")
  }
  { print }
' | awk '
  NF == 0 { blank++; if (blank > 1) next; print; next }
  { blank = 0; print }
'
