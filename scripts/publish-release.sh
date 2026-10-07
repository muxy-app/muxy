#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <X.Y.Z | X.Y.Z-beta.N> <artifact-directory>" >&2
    echo "A stable release also needs BETA_TAG, the beta it promotes, and DRAFT=true to stay a draft." >&2
    exit 1
fi
VERSION="$1"
CHANNEL="$(python3 "$ROOT/scripts/release.py" channel "$VERSION")"
ARTIFACTS="$(cd "$2" && pwd)"
TAG="v$VERSION"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_SHA:?GITHUB_SHA is required}"
if [[ "${GITHUB_REF:-}" != refs/heads/main ]]; then
    echo "Error: releases are restricted to main" >&2
    exit 1
fi
cd "$ROOT"
# A beta is built from the commit that triggered it; a stable release from the beta it promotes.
SOURCE="$GITHUB_SHA"
if [[ "$CHANNEL" == stable ]]; then
    : "${BETA_TAG:?BETA_TAG is required for a stable release}"
    SOURCE="$(git rev-parse --verify "refs/tags/$BETA_TAG^{commit}")"
fi
if [[ "$(git rev-parse HEAD)" != "$SOURCE" ]]; then
    echo "Error: checkout does not match the release source" >&2
    exit 1
fi
if [[ "$CHANNEL" == beta && "$(python3 scripts/release.py version | sed -n 's/^version=//p')" != "$VERSION" ]]; then
    echo "Error: version does not match the triggering commit count" >&2
    exit 1
fi

check_source() {
    git fetch origin '+refs/heads/main:refs/remotes/origin/main' --tags
    git merge-base --is-ancestor "$SOURCE" origin/main
    if git show-ref --verify --quiet "refs/tags/$TAG"; then
        if [[ "$(git rev-parse "refs/tags/$TAG^{}")" != "$SOURCE" ]]; then
            echo "Error: $TAG already points to a different commit" >&2
            exit 1
        fi
    fi
}
check_source

cd "$ARTIFACTS"
ASSETS=(install-muxy.sh)
MACOS_ARCHITECTURES='Apple Silicon (`arm64`)'
for ARCH in arm64 x86_64; do
    ASSETS+=("muxy-${VERSION}-linux-${ARCH}.tar.gz")
    # A beta may skip Intel; a stable release is also how Intel Macs leave Muxy 1.x.
    if [[ "$CHANNEL" == beta && "$ARCH" == x86_64 && ! -e "Muxy-${VERSION}-${ARCH}.dmg" && ! -e "muxy-${VERSION}-macos-${ARCH}.zip" ]]; then
        continue
    fi
    ASSETS+=("Muxy-${VERSION}-${ARCH}.dmg" "muxy-${VERSION}-macos-${ARCH}.zip")
    if [[ "$CHANNEL" == stable ]]; then
        ASSETS+=("appcast-${ARCH}.xml")
    fi
    if [[ "$ARCH" == x86_64 ]]; then
        MACOS_ARCHITECTURES+=' or Intel (`x86_64`)'
    fi
done
if [[ "$CHANNEL" == stable ]]; then
    # Muxy 1.x builds from before per-architecture feeds read appcast.xml.
    if [[ -f appcast-arm64.xml ]]; then
        cp appcast-arm64.xml appcast.xml
    fi
    ASSETS+=(appcast.xml)
fi
ASSETS+=("muxy-mobile-${VERSION}-ios.zip" "muxy-mobile-${VERSION}-android.zip" "muxy-mobile-${VERSION}.json")
for ASSET in "${ASSETS[@]}"; do
    if [[ ! -f "$ASSET" || ! -s "$ASSET" || -L "$ASSET" ]]; then
        echo "Error: missing regular release asset: $ASSET" >&2
        exit 1
    fi
done
python3 "$ROOT/scripts/release.py" update "$VERSION" "$GITHUB_REPOSITORY" "$ARTIFACTS"
ASSETS+=(update.json)
shasum -a 256 "${ASSETS[@]}" > SHA256SUMS
ASSETS+=(SHA256SUMS)

PRERELEASE=false
if [[ "$CHANNEL" == beta ]]; then
    PRERELEASE=true
fi
if gh release view "$TAG" --repo "$GITHUB_REPOSITORY" \
    --json isDraft,isPrerelease,targetCommitish > release.json; then
    python3 -c 'import json, sys; r = json.load(sys.stdin); sys.exit(0 if r["isPrerelease"] == (sys.argv[2] == "true") and r["targetCommitish"] == sys.argv[1] else 1)' \
        "$SOURCE" "$PRERELEASE" < release.json
    if [[ "$(python3 -c 'import json, sys; print(json.load(sys.stdin)["isDraft"])' < release.json)" == False ]]; then
        echo "==> $TAG is already published; leaving its assets unchanged"
        if [[ "$CHANNEL" == beta ]]; then
            python3 "$ROOT/scripts/publish-update.py" "$VERSION"
        fi
        exit 0
    fi
else
    INSTALL_CLI="\`\`\`sh
curl -fsSL https://github.com/$GITHUB_REPOSITORY/releases/download/$TAG/install-muxy.sh | sh -s -- --version $VERSION
\`\`\`

The installer uses \`~/.local/bin\`. Use \`--install-dir PATH\` to choose another directory and \`--replace\` to replace existing commands, including a desktop bundle link. It does not modify shell profiles or restart servers. Run \`muxy\` to open the TUI; Ctrl-B then D detaches."
    if [[ "$CHANNEL" == beta ]]; then
        PREVIOUS="$(git -C "$ROOT" describe --tags --match "v${VERSION%%.*}.*-beta*" --abbrev=0 "$SOURCE^" 2>/dev/null || true)"
        UPGRADE=""
        if [[ "$PREVIOUS" == v2.0.0-beta-* ]]; then
            UPGRADE="Betas numbered \`2.0.0-beta-N\` can't update to this numbering on their own: install this release over them once, and later betas update automatically.

"
        fi
        cat > release-notes.md <<EOF
${UPGRADE}Experimental Rust/GPUI beta from the \`main\` branch. Not intended for production use.

- macOS 14 or newer on $MACOS_ARCHITECTURES.
- Drag \`Muxy Beta.app\` to Applications. The app bundles the matching \`muxy\` CLI/TUI and \`muxy-server\`. Use **Install Command Line Tool** to expose the bundled CLI on PATH.
- Installs alongside Muxy, with separate settings and sessions in \`~/Library/Application Support/Muxy Beta\`.
- Newer 2.x betas download automatically. Use **Check for Updates…** or **Restart to Update…** to install. Compatible servers keep running during updates; incompatible updates wait for the existing restart flow.
- When replacing a beta manually, stop its server in Settings before replacing the app.

Standalone CLI/TUI and server: macOS 14+ on $MACOS_ARCHITECTURES, or Linux with glibc 2.35+ on ARM64 or x86_64. Install the exact matching pair without a desktop app, Rust, or Zig:

$INSTALL_CLI

Source: https://github.com/$GITHUB_REPOSITORY/commit/$SOURCE

EOF
    else
        cat > release-notes.md <<EOF
- macOS 14 or newer on $MACOS_ARCHITECTURES.
- Drag \`Muxy.app\` to Applications, or run \`brew install --cask muxy-app/tap/muxy\`. The app bundles the matching \`muxy\` CLI/TUI and \`muxy-server\`. Use **Install Command Line Tool** to expose the bundled CLI on PATH.
- Muxy 1.x updates itself to this release. Muxy 2 starts fresh, with its settings and sessions in \`~/Library/Application Support/Muxy 2\`. Muxy 1.x data stays where it is: **Settings → Backup & Restore → Import installed 1.x** brings it over.
- Newer releases download automatically. Use **Check for Updates…** or **Restart to Update…** to install.

Standalone CLI/TUI and server: macOS 14+ on $MACOS_ARCHITECTURES, or Linux with glibc 2.35+ on ARM64 or x86_64. Install them with \`brew install muxy-app/tap/muxy-cli\`, or without Homebrew:

$INSTALL_CLI

Source: https://github.com/$GITHUB_REPOSITORY/commit/$SOURCE, promoted from $BETA_TAG

EOF
        PREVIOUS="$(git -C "$ROOT" describe --tags --match "v${VERSION%%.*}.*" --exclude '*-*' --abbrev=0 "$SOURCE^" 2>/dev/null || true)"
    fi
    if [[ -n "$PREVIOUS" ]]; then
        gh api --method POST "repos/$GITHUB_REPOSITORY/releases/generate-notes" \
            -f "tag_name=$TAG" -f "target_commitish=$SOURCE" \
            -f "previous_tag_name=$PREVIOUS" --jq .body >> release-notes.md
    fi
    DRAFT_FLAGS=(--draft --latest=false)
    if [[ "$CHANNEL" == beta ]]; then
        DRAFT_FLAGS+=(--prerelease)
    fi
    gh release create "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --title "Muxy $VERSION" "${DRAFT_FLAGS[@]}" --notes-file release-notes.md
fi

# A failed upload leaves a resumable draft, never a half-populated public release.
gh release upload "$TAG" --repo "$GITHUB_REPOSITORY" --clobber "${ASSETS[@]}"
gh release view "$TAG" --repo "$GITHUB_REPOSITORY" --json assets > uploaded-assets.json
python3 - uploaded-assets.json "${ASSETS[@]}" <<'PY'
import json, pathlib, sys
remote = json.loads(pathlib.Path(sys.argv[1]).read_text())["assets"]
for name in sys.argv[2:]:
    matches = [asset for asset in remote if asset["name"] == name]
    if len(matches) != 1 or matches[0]["size"] != pathlib.Path(name).stat().st_size:
        sys.exit(f"Error: missing or incomplete uploaded asset: {name}")
PY
cd "$ROOT"
check_source
if [[ "$CHANNEL" == beta ]]; then
    gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --draft=false --prerelease --latest=false
    python3 "$ROOT/scripts/publish-update.py" "$VERSION"
elif [[ "${DRAFT:-false}" == true ]]; then
    echo "==> $TAG is a complete draft. Publishing it as the latest release updates Muxy 1.x and 2.x."
else
    # The latest release is the stable feed of Muxy 2.x and of Muxy 1.x's updater.
    gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --draft=false --prerelease=false --latest
fi
