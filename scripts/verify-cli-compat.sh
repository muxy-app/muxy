#!/usr/bin/env bash
set -euo pipefail

printf 'error: app-launching E2E verification is disabled\n' >&2
printf 'Use headless CLI contract tests and ask the user to verify native app behavior manually.\n' >&2
exit 1
