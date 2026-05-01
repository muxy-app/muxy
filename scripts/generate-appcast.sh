#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ $# -lt 3 ]]; then
  echo "Usage: $0 <dmg> <tag> <build-number> [output-path]" >&2
  echo "Env: SPARKLE_PRIVATE_KEY (required), CHANNEL (stable|beta, default stable)," >&2
  echo "     EXISTING_APPCAST (optional path to a previous appcast to prepend into)" >&2
  exit 1
fi

DMG="$1"
TAG="$2"
BUILD_NUMBER="$3"
OUT_PATH="${4:-appcast.xml}"
CHANNEL="${CHANNEL:-stable}"

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

NEW_ITEM_FILE=$(mktemp)
trap 'rm -f "$NEW_ITEM_FILE"' EXIT
cat > "$NEW_ITEM_FILE" << EOF
    <item>
      <title>Version ${VERSION}</title>
      <pubDate>${PUB_DATE}</pubDate>
      <sparkle:channel>${CHANNEL}</sparkle:channel>
      <sparkle:version>${BUILD_NUMBER}</sparkle:version>
      <sparkle:shortVersionString>${VERSION}</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>14.0</sparkle:minimumSystemVersion>
      <sparkle:fullReleaseNotesLink>https://github.com/muxy-app/muxy/releases/tag/${TAG}</sparkle:fullReleaseNotesLink>
      <enclosure url="${DOWNLOAD_URL_PREFIX}${FILENAME}" sparkle:edSignature="${SIG}" length="${SIZE}" type="application/octet-stream" />
    </item>
EOF

if [[ -n "${EXISTING_APPCAST:-}" && -f "$EXISTING_APPCAST" ]]; then
  echo "==> Merging into existing appcast: $EXISTING_APPCAST"
  VERSION="$VERSION" python3 - "$EXISTING_APPCAST" "$OUT_PATH" "$NEW_ITEM_FILE" <<'PYEOF'
import os, sys
src, dst, item_path = sys.argv[1], sys.argv[2], sys.argv[3]
version = os.environ["VERSION"]
with open(src, "r", encoding="utf-8") as f:
    data = f.read()
with open(item_path, "r", encoding="utf-8") as f:
    new_item = f.read()
marker = f"<sparkle:shortVersionString>{version}</sparkle:shortVersionString>"
if marker in data:
    sys.stderr.write(f"==> Version {version} already in appcast, skipping insert\n")
    out = data
else:
    idx = data.find("<item>")
    if idx == -1:
        idx = data.find("</channel>")
        out = data[:idx] + new_item + data[idx:]
    else:
        out = data[:idx] + new_item.lstrip() + "    " + data[idx:]
with open(dst, "w", encoding="utf-8") as f:
    f.write(out)
PYEOF
else
  NEW_ITEM=$(cat "$NEW_ITEM_FILE")
  cat > "$OUT_PATH" << EOF
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>Muxy Updates (${CHANNEL})</title>
    <link>https://github.com/muxy-app/muxy</link>
    <description>Updates for Muxy (${CHANNEL} channel)</description>
    <language>en</language>
${NEW_ITEM}
  </channel>
</rss>
EOF
fi

if grep -q 'sparkle:edSignature' "$OUT_PATH"; then
  echo "==> Generated appcast at $OUT_PATH (channel=$CHANNEL, verified: contains edSignature)"
else
  echo "ERROR: appcast is missing sparkle:edSignature!" >&2
  exit 1
fi
