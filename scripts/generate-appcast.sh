#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ $# -lt 3 ]]; then
  echo "Usage: $0 <dmg> <tag> <build-number> [output-path]" >&2
  exit 1
fi

DMG="$1"
TAG="$2"
BUILD_NUMBER="$3"
OUT_PATH="${4:-appcast.xml}"

if [[ -z "${SPARKLE_PRIVATE_KEY:-}" ]]; then
  echo "SPARKLE_PRIVATE_KEY is required." >&2
  exit 1
fi

SIGN_UPDATE="$PROJECT_ROOT/.build/artifacts/sparkle/Sparkle/bin/sign_update"
if [[ ! -x "$SIGN_UPDATE" ]]; then
  echo "Error: sign_update not found at $SIGN_UPDATE (run 'swift package resolve' first)" >&2
  exit 1
fi

DOWNLOAD_URL_PREFIX="${DOWNLOAD_URL_PREFIX:-https://github.com/muxy-app/muxy/releases/download/$TAG/}"

VERSION="${TAG#v}"
SIG=$(echo "$SPARKLE_PRIVATE_KEY" | "$SIGN_UPDATE" --ed-key-file - -p "$DMG")
SIZE=$(stat -f%z "$DMG")
FILENAME=$(basename "$DMG")
PUB_DATE=$(date -u "+%a, %d %b %Y %H:%M:%S %z")

cat > "$OUT_PATH" << EOF
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>Muxy Updates</title>
    <link>https://github.com/muxy-app/muxy</link>
    <description>Updates for Muxy</description>
    <language>en</language>
    <item>
      <title>Version ${VERSION}</title>
      <pubDate>${PUB_DATE}</pubDate>
      <sparkle:version>${BUILD_NUMBER}</sparkle:version>
      <sparkle:shortVersionString>${VERSION}</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>14.0</sparkle:minimumSystemVersion>
      <sparkle:fullReleaseNotesLink>https://github.com/muxy-app/muxy/releases/tag/${TAG}</sparkle:fullReleaseNotesLink>
      <enclosure url="${DOWNLOAD_URL_PREFIX}${FILENAME}" sparkle:edSignature="${SIG}" length="${SIZE}" type="application/octet-stream" />
    </item>
  </channel>
</rss>
EOF

if grep -q 'sparkle:edSignature' "$OUT_PATH"; then
  echo "==> Generated appcast at $OUT_PATH (verified: contains edSignature)"
else
  echo "ERROR: appcast is missing sparkle:edSignature!" >&2
  exit 1
fi
