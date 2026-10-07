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
    # Like Muxy 1.x: GitHub's generated changes, cleaned. A beta lists the changes since the
    # previous beta; a stable release lets GitHub choose the previous release.
    NOTES_ARGS=(-f "tag_name=$TAG" -f "target_commitish=$SOURCE")
    : > release-notes.md
    if [[ "$CHANNEL" == beta ]]; then
        PREVIOUS="$(git -C "$ROOT" describe --tags --match "v${VERSION%%.*}.*-beta*" --abbrev=0 "$SOURCE^" 2>/dev/null || true)"
        if [[ -n "$PREVIOUS" ]]; then
            NOTES_ARGS+=(-f "previous_tag_name=$PREVIOUS")
        fi
        if [[ "$PREVIOUS" == v2.0.0-beta-* ]]; then
            printf '%s\n\n' "Betas numbered \`2.0.0-beta-N\` can't update to this numbering on their own: install this release over them once, and later betas update automatically." > release-notes.md
        fi
    fi
    gh api --method POST "repos/$GITHUB_REPOSITORY/releases/generate-notes" "${NOTES_ARGS[@]}" --jq .body \
        | bash "$ROOT/scripts/clean-changelog.sh" >> release-notes.md
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
