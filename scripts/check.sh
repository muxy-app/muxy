#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT

command -v cargo >/dev/null 2>&1 || {
    printf 'error: cargo is required\n' >&2
    exit 1
}

cd "$PROJECT_ROOT"

export CARGO_NET_OFFLINE=true

printf '==> Checking shell syntax\n'
bash -n scripts/*.sh
if command -v shellcheck >/dev/null 2>&1; then
    shellcheck scripts/*.sh
fi

printf '==> Checking for comments\n'
command -v rg >/dev/null 2>&1 || {
    printf 'error: ripgrep is required for the comment and boundary gates\n' >&2
    exit 1
}
if rg -n --glob '!target' '^\s*//|^\s*/\*|\s//\s' crates/; then
    printf 'error: comments are not permitted under crates/\n' >&2
    exit 1
fi
if rg -n '^\s*#' scripts/ | rg -v '^[^:]+:1:#!/'; then
    printf 'error: comments are not permitted under scripts/\n' >&2
    exit 1
fi

printf '==> Checking superseded paths\n'
for superseded in chrome ui store prefs workspace workspace_store.rs shortcuts.rs; do
    if [[ -e "crates/muxy/src/$superseded" ]]; then
        printf 'error: superseded path exists: crates/muxy/src/%s\n' "$superseded" >&2
        exit 1
    fi
done

printf '==> Checking crate boundaries\n'
check_boundary() {
    local crate="$1" pattern="$2"
    if rg -n --glob '!target' "$pattern" "crates/$crate"; then
        printf 'error: %s must not mention %s\n' "$crate" "$pattern" >&2
        exit 1
    fi
}
check_boundary muxy-api 'gpui|objc2|ghostty|muxy[-_]terminal|muxy[-_]ui'
check_boundary muxy-core 'gpui|muxy[-_]api|muxy[-_]terminal|muxy[-_]ui|notify'
check_boundary muxy-terminal 'gpui|muxy[-_]api|muxy[-_]ui'
check_boundary muxy-ui 'muxy[-_]core|muxy[-_]api|muxy[-_]terminal|ghostty'

printf '==> Checking the terminal backend boundary\n'
if rg -n 'objc2|ghostty_host|ghostty_sys|NativeView|NSView' \
    crates/muxy/src/views crates/muxy-core/src/workspace; then
    printf 'error: views and workspace must not name backend types\n' >&2
    exit 1
fi

printf '==> Checking Rust formatting\n'
cargo fmt --all -- --check

printf '==> Running Clippy\n'
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings

printf '==> Building workspace\n'
cargo build --workspace --all-targets --all-features --locked --offline

printf '==> Running tests\n'
cargo test --workspace --all-targets --all-features --locked --offline

if [[ "$(uname -s)" == "Darwin" ]]; then
    printf '==> Building debug application bundle\n'
    "$SCRIPT_DIR/build-app.sh" debug

    printf '==> Verifying debug application bundle\n'
    "$SCRIPT_DIR/verify-bundle.sh" "$PROJECT_ROOT/target/debug/Muxy.app"
fi
