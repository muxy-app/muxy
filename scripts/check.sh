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
bash -n scripts/*.sh resources/muxy-dev-bin/muxy \
    resources/muxy-shell-integration/bash resources/muxy-shell-integration/zsh
if command -v shellcheck >/dev/null 2>&1; then
    shellcheck scripts/*.sh resources/muxy-dev-bin/muxy
    shellcheck -s bash resources/muxy-shell-integration/bash resources/muxy-shell-integration/zsh
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

printf '==> Checking production defaults isolation\n'
production_defaults_domain='com.muxy.'
production_defaults_domain+='app'
production_defaults_matches="$(rg -n -F "$production_defaults_domain" crates/ \
    scripts/stage-test-app.sh scripts/run-test-app.sh \
    | rg -v '^crates/muxy-core/src/migration.rs:' || true)"
if [[ -n "$production_defaults_matches" ]]; then
    printf '%s\n' "$production_defaults_matches"
    printf 'error: only the migration reader may select the production defaults domain\n' >&2
    exit 1
fi

printf '==> Checking migration ownership\n'
migration_owner='crates/muxy-core/src/migration.rs'
migration_matches="$(rg -n 'NSUserDefaults|swift-profile-migration\.json|MUXY_TEST_SWIFT_' crates/ \
    | rg -v "^${migration_owner}:" || true)"
if [[ -n "$migration_matches" ]]; then
    printf '%s\n' "$migration_matches"
    printf 'error: migration-only state escaped %s\n' "$migration_owner" >&2
    exit 1
fi
if rg -n 'Library/Preferences|plist::' crates/muxy-core/src \
    || rg -n '^plist\.workspace' crates/muxy-core/Cargo.toml; then
    printf 'error: normal core preferences must remain portable\n' >&2
    exit 1
fi

printf '==> Checking environment policy ownership\n'
environment_owner='crates/muxy-core/src/environment.rs'
policy_literals=(
    'muxy-dev.sock'
    'muxy.sock'
    'sessions-dev'
    'hooks-dev'
    'app.muxy.mobile.serverEnabled.dev'
    'app.muxy.mobile.serverPort.dev'
    'app.muxy.mobile.scrollbackCap.dev'
    'app.muxy.mobile.serverEnabled'
    'app.muxy.mobile.serverPort'
    'app.muxy.mobile.scrollbackCap'
)
for literal in "${policy_literals[@]}"; do
    matches="$(rg -n -F --glob '*.rs' "$literal" crates/ \
        | rg -v "^${environment_owner}:" || true)"
    if [[ -n "$matches" ]]; then
        printf '%s\n' "$matches"
        printf 'error: environment policy literal is owned by %s: %s\n' \
            "$environment_owner" "$literal" >&2
        exit 1
    fi
done
fallback_pattern='"muxy-dev-(\{[^}]*\}|[0-9]+)|"muxy-(\{[^}]*\}|[0-9]+)'
fallback_matches="$(rg -n --glob '*.rs' "$fallback_pattern" crates/ \
    | rg -v "^${environment_owner}:" || true)"
if [[ -n "$fallback_matches" ]]; then
    printf '%s\n' "$fallback_matches"
    printf 'error: fallback directory format is owned by %s\n' "$environment_owner" >&2
    exit 1
fi
debug_selector_matches="$(rg -n --glob '*.rs' 'debug_assertions' crates/ \
    | rg -v "^${environment_owner}:" || true)"
if [[ -n "$debug_selector_matches" ]]; then
    printf '%s\n' "$debug_selector_matches"
    printf 'error: debug_assertions is owned by %s\n' "$environment_owner" >&2
    exit 1
fi
removed_hook_flag='FF_AI_'
removed_hook_flag+='HOOKS'
if rg -n -F "$removed_hook_flag" crates/ scripts/ PLAN.md ARCHITECTURE.md; then
    printf 'error: removed AI hook flag remains in the Rust design\n' >&2
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
check_boundary muxy-proto 'gpui|objc2|objective-c|ghostty|muxy[-_](core|api|terminal|ui)|muxy::|package\s*=\s*"muxy"|^\s*muxy(\.workspace)?\s*='
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
    printf '==> Checking staged migration safety guards\n'
    "$SCRIPT_DIR/verify-p2-5-migration.sh" --self-test

    printf '==> Building debug application bundle\n'
    "$SCRIPT_DIR/build-app.sh" debug

    printf '==> Verifying debug application bundle\n'
    "$SCRIPT_DIR/verify-bundle.sh" "$PROJECT_ROOT/target/debug/Muxy.app" debug
fi
