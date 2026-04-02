#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 ]]; then
  echo "Usage: $0 <arm64-dmg> <x86_64-dmg> <tag> [output-path]" >&2
  exit 1
fi

ARM64_DMG="$1"
X86_DMG="$2"
TAG="$3"
OUT_PATH="${4:-appcast.xml}"

if [[ -z "${SPARKLE_PRIVATE_KEY:-}" ]]; then
  echo "SPARKLE_PRIVATE_KEY is required." >&2
  exit 1
fi

SPARKLE_VERSION="${SPARKLE_VERSION:-2.9.1}"
DOWNLOAD_URL_PREFIX="${DOWNLOAD_URL_PREFIX:-https://github.com/muxy-app/muxy/releases/download/$TAG/}"

work_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT

echo "==> Cloning Sparkle ${SPARKLE_VERSION}..."
git clone --depth 1 --branch "$SPARKLE_VERSION" https://github.com/sparkle-project/Sparkle "$work_dir/Sparkle"

echo "==> Building sign_update tool..."
xcodebuild \
  -project "$work_dir/Sparkle/Sparkle.xcodeproj" \
  -scheme sign_update \
  -configuration Release \
  -derivedDataPath "$work_dir/build" \
  CODE_SIGNING_ALLOWED=NO \
  build >/dev/null

sign_update="$work_dir/build/Build/Products/Release/sign_update"

if [[ ! -x "$sign_update" ]]; then
  echo "sign_update binary not found at $sign_update" >&2
  exit 1
fi

key_file="$work_dir/sparkle_ed_key"
padded_key="$SPARKLE_PRIVATE_KEY"
while (( ${#padded_key} % 4 != 0 )); do
  padded_key="${padded_key}="
done
printf "%s" "$padded_key" > "$key_file"

VERSION="${TAG#v}"
ARM64_SIG=$("$sign_update" -p --ed-key-file "$key_file" "$ARM64_DMG")
X86_SIG=$("$sign_update" -p --ed-key-file "$key_file" "$X86_DMG")
ARM64_SIZE=$(stat -f%z "$ARM64_DMG")
X86_SIZE=$(stat -f%z "$X86_DMG")
ARM64_FILENAME=$(basename "$ARM64_DMG")
X86_FILENAME=$(basename "$X86_DMG")
PUB_DATE=$(date -R)

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
      <sparkle:version>${VERSION}</sparkle:version>
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
