#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [[ $# -lt 4 ]]; then
  echo "Usage: $0 <arm64-dmg> <x86_64-dmg> <tag> <build-number> [output-path]" >&2
  exit 1
fi

ARM64_DMG="$1"
X86_DMG="$2"
TAG="$3"
BUILD_NUMBER="$4"
OUT_PATH="${5:-appcast.xml}"

if [[ -z "${SPARKLE_PRIVATE_KEY:-}" ]]; then
  echo "SPARKLE_PRIVATE_KEY is required." >&2
  exit 1
fi

DOWNLOAD_URL_PREFIX="${DOWNLOAD_URL_PREFIX:-https://github.com/muxy-app/muxy/releases/download/$TAG/}"

VERSION="${TAG#v}"
ARM64_SIG=$(swift "$SCRIPT_DIR/sign-ed25519.swift" "$SPARKLE_PRIVATE_KEY" "$ARM64_DMG")
X86_SIG=$(swift "$SCRIPT_DIR/sign-ed25519.swift" "$SPARKLE_PRIVATE_KEY" "$X86_DMG")
ARM64_SIZE=$(stat -f%z "$ARM64_DMG")
X86_SIZE=$(stat -f%z "$X86_DMG")
ARM64_FILENAME=$(basename "$ARM64_DMG")
X86_FILENAME=$(basename "$X86_DMG")
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
      <sparkle:fullReleaseNotesLink>https://github.com/muxy-app/muxy/releases/tag/${TAG}</sparkle:fullReleaseNotesLink>
      <enclosure url="${DOWNLOAD_URL_PREFIX}${ARM64_FILENAME}" sparkle:edSignature="${ARM64_SIG}" length="${ARM64_SIZE}" type="application/octet-stream" sparkle:os="macos" sparkle:installationType="dmg" sparkle:arch="arm64" />
      <enclosure url="${DOWNLOAD_URL_PREFIX}${X86_FILENAME}" sparkle:edSignature="${X86_SIG}" length="${X86_SIZE}" type="application/octet-stream" sparkle:os="macos" sparkle:installationType="dmg" sparkle:arch="x86_64" />
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
