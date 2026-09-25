#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <2.0.0-beta-N> <artifact-directory>" >&2
    exit 1
fi
VERSION="$1"
python3 "$ROOT/scripts/beta_release.py" check-version "$VERSION"
ARTIFACTS="$(cd "$2" && pwd)"
TAG="v$VERSION"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_SHA:?GITHUB_SHA is required}"
if [[ "${GITHUB_REF:-}" != refs/heads/2.x ]]; then
    echo "Error: beta releases are restricted to 2.x" >&2
    exit 1
fi
cd "$ROOT"
if [[ "$(git rev-parse HEAD)" != "$GITHUB_SHA" ]]; then
    echo "Error: checkout does not match the triggering commit" >&2
    exit 1
fi
if [[ "$(python3 scripts/beta_release.py version | sed -n 's/^version=//p')" != "$VERSION" ]]; then
    echo "Error: version does not match the triggering commit count" >&2
    exit 1
fi

check_source() {
    git fetch origin '+refs/heads/2.x:refs/remotes/origin/2.x' --tags
    git merge-base --is-ancestor "$GITHUB_SHA" origin/2.x
    if git show-ref --verify --quiet "refs/tags/$TAG"; then
        if [[ "$(git rev-parse "refs/tags/$TAG^{}")" != "$GITHUB_SHA" ]]; then
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
    if [[ "$ARCH" == x86_64 && ! -e "Muxy-${VERSION}-${ARCH}.dmg" && ! -e "muxy-${VERSION}-macos-${ARCH}.zip" ]]; then
        continue
    fi
    ASSETS+=("Muxy-${VERSION}-${ARCH}.dmg" "muxy-${VERSION}-macos-${ARCH}.zip")
    if [[ "$ARCH" == x86_64 ]]; then
        MACOS_ARCHITECTURES+=' or Intel (`x86_64`)'
    fi
done
ASSETS+=("muxy-mobile-${VERSION}-ios.zip" "muxy-mobile-${VERSION}-android.zip" "muxy-mobile-${VERSION}.json")
for ASSET in "${ASSETS[@]}"; do
    if [[ ! -f "$ASSET" || ! -s "$ASSET" || -L "$ASSET" ]]; then
        echo "Error: missing regular release asset: $ASSET" >&2
        exit 1
    fi
done
python3 "$ROOT/scripts/beta_release.py" update "$VERSION" "$GITHUB_REPOSITORY" "$ARTIFACTS"
ASSETS+=(update.json)
shasum -a 256 "${ASSETS[@]}" > SHA256SUMS
ASSETS+=(SHA256SUMS)

if gh release view "$TAG" --repo "$GITHUB_REPOSITORY" \
    --json isDraft,isPrerelease,targetCommitish > release.json; then
    python3 -c 'import json, sys; r = json.load(sys.stdin); sys.exit(0 if r["isPrerelease"] and r["targetCommitish"] == sys.argv[1] else 1)' \
        "$GITHUB_SHA" < release.json
    if [[ "$(python3 -c 'import json, sys; print(json.load(sys.stdin)["isDraft"])' < release.json)" == False ]]; then
        echo "==> $TAG is already published; leaving its assets unchanged"
        python3 "$ROOT/scripts/publish-update.py" "$VERSION"
        exit 0
    fi
else
    cat > release-notes.md <<EOF
Experimental Rust/GPUI beta from the \`2.x\` branch. Not intended for production use.

- macOS 14 or newer on $MACOS_ARCHITECTURES.
- Drag \`Muxy Beta.app\` to Applications. The app bundles the matching \`muxy\` CLI/TUI and \`muxy-server\`. Use **Install Command Line Tool** to expose the bundled CLI on PATH.
- Installs alongside Muxy, with separate settings and sessions in \`~/Library/Application Support/Muxy Beta\`.
- Newer 2.x betas download automatically. Use **Check for Updates…** or **Restart to Update…** to install. Compatible servers keep running during updates; incompatible updates wait for the existing restart flow.
- When replacing a beta manually, stop its server in Settings before replacing the app.

Standalone CLI/TUI and server: macOS 14+ on $MACOS_ARCHITECTURES, or Linux with glibc 2.35+ on ARM64 or x86_64. Install the exact matching pair without a desktop app, Rust, or Zig:

\`\`\`sh
curl -fsSL https://github.com/$GITHUB_REPOSITORY/releases/download/$TAG/install-muxy.sh | sh -s -- --version $VERSION
\`\`\`

The installer uses \`~/.local/bin\`. Use \`--install-dir PATH\` to choose another directory and \`--replace\` to replace existing commands, including a desktop bundle link. It does not modify shell profiles or restart servers. Run \`muxy\` to open the TUI; Ctrl-B then D detaches.

Source: https://github.com/$GITHUB_REPOSITORY/commit/$GITHUB_SHA

EOF
    PREVIOUS="$(git -C "$ROOT" describe --tags --match 'v2.0.0-beta-*' --match 'v2.0.0-alpha-*' --abbrev=0 "$GITHUB_SHA^" 2>/dev/null || true)"
    if [[ -n "$PREVIOUS" ]]; then
        gh api --method POST "repos/$GITHUB_REPOSITORY/releases/generate-notes" \
            -f "tag_name=$TAG" -f "target_commitish=$GITHUB_SHA" \
            -f "previous_tag_name=$PREVIOUS" --jq .body >> release-notes.md
    fi
    gh release create "$TAG" --repo "$GITHUB_REPOSITORY" --target "$GITHUB_SHA" \
        --title "Muxy $VERSION" --draft --prerelease --latest=false --notes-file release-notes.md
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
gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --target "$GITHUB_SHA" \
    --draft=false --prerelease --latest=false
python3 "$ROOT/scripts/publish-update.py" "$VERSION"
