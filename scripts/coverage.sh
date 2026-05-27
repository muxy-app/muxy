#!/usr/bin/env bash
set -euo pipefail

MIN_LINE_COVERAGE=${MIN_LINE_COVERAGE:-90}
BUILD_DIR=".build/arm64-apple-macosx/debug"
TEST_BINARY="$BUILD_DIR/MuxyPackageTests.xctest/Contents/MacOS/MuxyPackageTests"
PROFILE="$BUILD_DIR/codecov/default.profdata"
IGNORE_REGEX='\.build|Tests|Muxy/Views|Muxy/Commands|Muxy/MuxyApp\.swift|Muxy/Services|MuxyServer|Muxy/Models/AppState\.swift|Muxy/Models/VCSTabState\.swift|Muxy/Models/VoiceRecordingState\.swift|Muxy/Extensions/View\+'

swift test --enable-code-coverage --parallel --num-workers 1

report=$(xcrun llvm-cov report "$TEST_BINARY" -instr-profile "$PROFILE" -ignore-filename-regex="$IGNORE_REGEX")
total_line=$(printf "%s\n" "$report" | awk '/^TOTAL[[:space:]]/ { print $(NF-3) }')
total_line=${total_line%\%}

printf "%s\n" "$report"
printf "\nLine coverage: %s%% (minimum %s%%)\n" "$total_line" "$MIN_LINE_COVERAGE"

awk -v actual="$total_line" -v minimum="$MIN_LINE_COVERAGE" 'BEGIN { exit(actual + 0 >= minimum + 0 ? 0 : 1) }'
