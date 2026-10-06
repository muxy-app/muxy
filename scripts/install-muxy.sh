#!/bin/sh
set -eu
umask 077
LC_ALL=C
export LC_ALL

fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "Install $1 and retry."; }
usage() { echo 'Usage: install-muxy.sh (--version 2.0.0-beta-N | --dev-archive < archive.tar.gz) [--install-dir PATH] [--replace]'; }
VERSION=
DEV_ARCHIVE=false
DEST=${HOME:?HOME is required}/.local/bin
REPLACE=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --version|--install-dir)
            [ "$#" -ge 2 ] && [ -n "$2" ] || fail "$1 requires a value"
            case "$1" in --version) VERSION=$2 ;; --install-dir) DEST=$2 ;; esac
            shift 2 ;;
        --replace) REPLACE=true; shift ;;
        --dev-archive) DEV_ARCHIVE=true; shift ;;
        --help) usage; exit 0 ;;
        *) usage >&2; fail "Unknown argument: $1" ;;
    esac
done
if [ "$DEV_ARCHIVE" = true ]; then
    [ -z "$VERSION" ] || fail 'Use --dev-archive without --version.'
    VERSION=dev
    FILES='muxy muxy-server'
    FILE_COUNT=2
else
    case "$VERSION" in
        2.0.0-beta-*) COUNT=${VERSION#2.0.0-beta-} ;;
        *) fail 'Specify an exact version with --version 2.0.0-beta-N.' ;;
    esac
    case "$COUNT" in ''|0*|*[!0-9]*) fail 'Invalid beta version.' ;; esac
    FILES='muxy muxy-server LICENSE'
    FILE_COUNT=3
    need curl
fi
for TOOL in uname mktemp mkdir rmdir rm mv cp ln readlink chmod cat awk sort cmp sed; do need "$TOOL"; done
case "$(uname -s)" in
    Darwin) PLATFORM=macos; EXT=zip; need unzip; MOVE_FLAG=-h ;;
    Linux)
        PLATFORM=linux; EXT=tar.gz; need tar; need getconf; MOVE_FLAG=-T
        GLIBC=$(getconf GNU_LIBC_VERSION 2>/dev/null) || fail 'Linux requires glibc 2.35 or newer (musl is unsupported).'
        printf '%s\n' "$GLIBC" | awk '
            $1 == "glibc" && $2 ~ /^[0-9]+\.[0-9]+$/ {
                split($2, v, "."); if (v[1] > 2 || (v[1] == 2 && v[2] >= 35)) ok=1
            } END { exit !ok }' || fail 'Linux requires glibc 2.35 or newer.'
        ;;
    *) fail 'Supported systems: macOS and glibc Linux.' ;;
esac
case "$(uname -m)" in
    x86_64|amd64) ARCH=x86_64 ;;
    arm64|aarch64) ARCH=arm64 ;;
    *) fail 'Supported architectures: x86_64 and ARM64.' ;;
esac
if [ "$DEV_ARCHIVE" = true ]; then
    [ "$PLATFORM-$ARCH" = linux-x86_64 ] || fail 'This development archive requires x86_64 Linux.'
elif command -v sha256sum >/dev/null 2>&1; then
    hash() { sha256sum "$1"; }
elif command -v shasum >/dev/null 2>&1; then
    hash() { shasum -a 256 "$1"; }
else
    fail 'Install sha256sum or shasum and retry.'
fi

ARCHIVE=muxy-$VERSION-$PLATFORM-$ARCH.$EXT
BASE=https://github.com/muxy-app/muxy/releases/download/v$VERSION
TEMP=$(mktemp -d "${TMPDIR:-/tmp}/muxy-install.XXXXXX")
LOCK=
STAGE=
GENERATION=
PREVIOUS=
COMMITTED=false
CHANGED=
cleanup() {
    if [ -n "$GENERATION" ] && [ -L "$MANAGED/current" ] &&
        [ "$(readlink "$MANAGED/current")" = "${GENERATION##*/}" ]; then
        COMMITTED=true
    fi
    if [ "$COMMITTED" = false ] && [ -n "$STAGE" ]; then
        for BINARY in $CHANGED; do
            if [ -e "$STAGE/old-$BINARY" ] || [ -L "$STAGE/old-$BINARY" ]; then
                mv -f "$MOVE_FLAG" "$STAGE/old-$BINARY" "$DEST/$BINARY" || true
            else
                rm -f "$DEST/$BINARY"
            fi
        done
        if [ -n "$PREVIOUS" ] && [ -L "$MANAGED/current" ] &&
            [ "$(readlink "$MANAGED/current")" = "${PREVIOUS##*/}" ]; then
            if [ -L "$STAGE/old-current" ]; then
                mv -f "$MOVE_FLAG" "$STAGE/old-current" "$MANAGED/current" || true
            else
                rm -f "$MANAGED/current"
            fi
        fi
        [ -z "$GENERATION" ] || rm -rf "$GENERATION"
    fi
    [ -z "$STAGE" ] || rm -rf "$STAGE"
    [ -z "$LOCK" ] || rmdir "$LOCK"
    rm -rf "$TEMP"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
if [ "$DEV_ARCHIVE" = true ]; then
    cat > "$TEMP/$ARCHIVE"
else
    curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 "$BASE/$ARCHIVE" -o "$TEMP/$ARCHIVE"
    curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 "$BASE/SHA256SUMS" -o "$TEMP/SHA256SUMS"
    EXPECTED=$(awk -v name="$ARCHIVE" '
        $2 == name && length($1) == 64 && $1 !~ /[^0-9a-f]/ { value=$1; count++ }
        END { if (count != 1) exit 1; print value }' "$TEMP/SHA256SUMS") || fail 'Missing or ambiguous archive checksum.'
    ACTUAL=$(hash "$TEMP/$ARCHIVE")
    [ "${ACTUAL%% *}" = "$EXPECTED" ] || fail 'Archive SHA-256 mismatch.'
fi

# Validate names and types before reading individual members into regular files.
printf '%s\n' $FILES | sort > "$TEMP/expected"
if [ "$EXT" = zip ]; then
    unzip -Z -1 "$TEMP/$ARCHIVE" > "$TEMP/members"
    unzip -Z -l "$TEMP/$ARCHIVE" > "$TEMP/types"
    awk '/^[-dlcbps]/ { if (substr($0,1,1) != "-") exit 1; count++ }
        END { if (count != 3) exit 1 }' "$TEMP/types" || fail 'Archive must contain three regular files.'
else
    tar -tzf "$TEMP/$ARCHIVE" > "$TEMP/members"
    tar -tvzf "$TEMP/$ARCHIVE" > "$TEMP/types"
    awk -v wanted="$FILE_COUNT" '{ if (substr($0,1,1) != "-") exit 1; count++ }
        END { if (count != wanted) exit 1 }' "$TEMP/types" || fail "Archive must contain $FILE_COUNT regular files."
fi
sort "$TEMP/members" > "$TEMP/sorted"
cmp -s "$TEMP/expected" "$TEMP/sorted" || fail 'Unexpected archive members.'

mkdir -p "$DEST"
DEST=$(cd "$DEST" && pwd -P)
MANAGED=$DEST/.muxy
[ ! -L "$MANAGED" ] || fail "$MANAGED must not be a symlink."
mkdir -p "$MANAGED"
chmod 700 "$MANAGED"
if ! mkdir "$MANAGED/install.lock" 2>/dev/null; then
    fail "Another installation owns $MANAGED/install.lock. If it was interrupted, remove that empty lock directory and retry."
fi
LOCK=$MANAGED/install.lock
STAGE=$(mktemp -d "$MANAGED/.stage.XXXXXX")
mkdir "$STAGE/pair"
for BINARY in $FILES; do
    if [ "$EXT" = zip ]; then
        unzip -p "$TEMP/$ARCHIVE" "$BINARY" > "$STAGE/pair/$BINARY"
    else
        tar -xOzf "$TEMP/$ARCHIVE" "$BINARY" > "$STAGE/pair/$BINARY"
    fi
    [ -s "$STAGE/pair/$BINARY" ] || fail "Empty archive member: $BINARY"
done
chmod 755 "$STAGE/pair/muxy" "$STAGE/pair/muxy-server"
[ "$DEV_ARCHIVE" = true ] || chmod 644 "$STAGE/pair/LICENSE"

quote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
replacement() {
    if [ "$DEV_ARCHIVE" = true ]; then
        printf 'To replace an existing development build, use install-muxy.sh --dev-archive --replace with the archive on stdin.\n' >&2
        return
    fi
    printf 'To replace the installed commands, run:\n  curl -fsSL %s/install-muxy.sh | sh -s -- --version %s --install-dir ' "$BASE" "$VERSION" >&2
    quote "$DEST" >&2
    printf ' --replace\n' >&2
}
for BINARY in muxy muxy-server; do
    if [ -e "$DEST/$BINARY" ] || [ -L "$DEST/$BINARY" ]; then
        [ -f "$DEST/$BINARY" ] || [ -L "$DEST/$BINARY" ] || fail "$DEST/$BINARY is not a file."
        if [ "$REPLACE" = false ]; then
            if [ ! -L "$DEST/$BINARY" ] && cmp -s "$DEST/$BINARY" "$STAGE/pair/$BINARY"; then
                continue
            fi
            replacement
            fail "$DEST/$BINARY already exists."
        fi
    fi
done
if [ ! -L "$DEST/muxy" ] && [ ! -L "$DEST/muxy-server" ] &&
    cmp -s "$DEST/muxy" "$STAGE/pair/muxy" && cmp -s "$DEST/muxy-server" "$STAGE/pair/muxy-server"; then
    echo "Muxy $VERSION is already installed."
    exit 0
fi
[ ! -e "$MANAGED/current" ] || [ -L "$MANAGED/current" ] || fail "$MANAGED/current must be a symlink."
GENERATION=$(mktemp -d "$MANAGED/pair-$VERSION.XXXXXX")
for BINARY in $FILES; do mv "$STAGE/pair/$BINARY" "$GENERATION/"; done
ln -s "${GENERATION##*/}" "$STAGE/current"

# Preserve a usable previous pair while converting ordinary files or bundle
# links. Public commands must never point at a missing or unrelated generation.
if [ -f "$DEST/muxy" ] && [ -f "$DEST/muxy-server" ] &&
    { [ ! -L "$DEST/muxy" ] || [ "$(readlink "$DEST/muxy")" != '.muxy/current/muxy' ] ||
      [ ! -L "$DEST/muxy-server" ] || [ "$(readlink "$DEST/muxy-server")" != '.muxy/current/muxy-server' ]; }; then
    PREVIOUS=$(mktemp -d "$MANAGED/pair-previous.XXXXXX")
    cp -pL "$DEST/muxy" "$DEST/muxy-server" "$PREVIOUS/"
    if [ -L "$MANAGED/current" ]; then
        cp -P "$MANAGED/current" "$STAGE/old-current"
    fi
    ln -s "${PREVIOUS##*/}" "$STAGE/previous"
    mv -f "$MOVE_FLAG" "$STAGE/previous" "$MANAGED/current"
fi

# Existing managed commands already use this shared pointer. For the first
# installation, prepare both public links with rollback before activating it.
for BINARY in muxy-server muxy; do
    if [ -L "$DEST/$BINARY" ] && [ "$(readlink "$DEST/$BINARY")" = ".muxy/current/$BINARY" ]; then
        continue
    fi
    if [ -e "$DEST/$BINARY" ] || [ -L "$DEST/$BINARY" ]; then
        cp -P "$DEST/$BINARY" "$STAGE/old-$BINARY"
    fi
    ln -s ".muxy/current/$BINARY" "$STAGE/$BINARY"
    CHANGED="$BINARY $CHANGED"
    mv -f "$MOVE_FLAG" "$STAGE/$BINARY" "$DEST/$BINARY"
done
mv -f "$MOVE_FLAG" "$STAGE/current" "$MANAGED/current"
COMMITTED=true
printf 'Installed Muxy %s in %s\n' "$VERSION" "$DEST"
case ":$PATH:" in
    *":$DEST:"*) ;;
    *) printf 'Add this directory to PATH: %s\n' "$DEST" ;;
esac
replacement
