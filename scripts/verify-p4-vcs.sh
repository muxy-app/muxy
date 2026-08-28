#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_PARENT="$PROJECT_ROOT/target/test-verification"
readonly VERIFICATION_ROOT="$VERIFICATION_PARENT/p4"
readonly OWNERSHIP_MARKER="$VERIFICATION_ROOT/.muxy-p4-verifier"
readonly OWNERSHIP_VALUE="muxy-p4-vcs-verifier-v1"
readonly SOURCE_CLI="$PROJECT_ROOT/Muxy/Resources/scripts/muxy-cli"

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

assert_safe_ancestors() {
    local path
    for path in "$PROJECT_ROOT/target" "$VERIFICATION_PARENT" "$VERIFICATION_ROOT"; do
        [[ ! -L "$path" ]] || return 1
        [[ ! -e "$path" || -d "$path" ]] || return 1
    done
    if [[ -d "$VERIFICATION_PARENT" ]]; then
        [[ "$(cd "$VERIFICATION_PARENT" && pwd -P)" == "$VERIFICATION_PARENT" ]] || return 1
    fi
}

path_is_safe() {
    local path="$1"
    local relative component cursor old_ifs
    [[ "$path" == "$VERIFICATION_ROOT"/* ]] || return 1
    [[ "$path" != *"/../"* && "$path" != */.. && "$path" != *"/./"* && "$path" != */. ]] || return 1
    relative="${path#"$VERIFICATION_ROOT"/}"
    [[ -n "$relative" ]] || return 1
    cursor="$VERIFICATION_ROOT"
    old_ifs="$IFS"
    IFS='/'
    read -r -a components <<< "$relative"
    IFS="$old_ifs"
    for component in "${components[@]}"; do
        [[ -n "$component" ]] || return 1
        cursor="$cursor/$component"
        [[ ! -L "$cursor" ]] || return 1
    done
}

ensure_root() {
    assert_safe_ancestors || fail "verification path has an unsafe ancestor"
    mkdir -p "$VERIFICATION_PARENT"
    if [[ -e "$VERIFICATION_ROOT" ]]; then
        [[ -f "$OWNERSHIP_MARKER" && ! -L "$OWNERSHIP_MARKER" ]] || {
            fail "verification root is not owned by this verifier"
        }
        [[ "$(<"$OWNERSHIP_MARKER")" == "$OWNERSHIP_VALUE" ]] || {
            fail "verification ownership marker differs"
        }
    else
        mkdir "$VERIFICATION_ROOT"
        printf '%s\n' "$OWNERSHIP_VALUE" > "$OWNERSHIP_MARKER"
    fi
    assert_safe_ancestors || fail "verification path changed while preparing it"
}

reset_safe_path() {
    local path="$1"
    ensure_root
    path_is_safe "$path" || fail "refusing unsafe verification path: $path"
    rm -rf "$path"
    mkdir -p "$path"
}

write_fake_provider() {
    local executable="$1"
    printf '%s\n' '#!/usr/bin/env bash' > "$executable"
    cat >> "$executable" <<'FAKE_PROVIDER'
set -euo pipefail
log="${MUXY_FAKE_LOG:?}"
count=$#
((count > 0))
prompt="${!count}"
printf "executable=%s\n" "$0" > "$log"
printf "cwd=%s\n" "$PWD" >> "$log"
printf "stdin_mode=closed\n" >> "$log"
printf "stdin_length=0\n" >> "$log"
printf "prompt_length=%s\n" "${#prompt}" >> "$log"
printf "env:MUXY_FIXTURE_ID=%s\n" "${MUXY_FIXTURE_ID:-}" >> "$log"
index=0
while (($# > 1)); do
    printf "argv[%s]=%s\n" "$index" "$1" >> "$log"
    index=$((index + 1))
    shift
done
printf "%s\n" "{\"message\":\"fixture commit\"}"
FAKE_PROVIDER
    chmod 0755 "$executable"
}

write_fake_gh() {
    local executable="$1"
    printf '%s\n' '#!/usr/bin/env bash' > "$executable"
    cat >> "$executable" <<'FAKE_GH'
set -euo pipefail
log="${MUXY_FAKE_LOG:?}"
printf "executable=%s\n" "$0" > "$log"
printf "cwd=%s\n" "$PWD" >> "$log"
printf "stdin_mode=closed\n" >> "$log"
printf "stdin_length=0\n" >> "$log"
printf "env:MUXY_FIXTURE_ID=%s\n" "${MUXY_FIXTURE_ID:-}" >> "$log"
index=0
for argument in "$@"; do
    printf "argv[%s]=%s\n" "$index" "$argument" >> "$log"
    index=$((index + 1))
done
printf "%s\n" "https://example.invalid/owner/repository/pull/42"
FAKE_GH
    chmod 0755 "$executable"
}

write_network_sentinel() {
    local executable="$1"
    printf '%s\n' '#!/usr/bin/env bash' > "$executable"
    cat >> "$executable" <<'NETWORK_SENTINEL'
set -euo pipefail
printf "%s\n" "$0 $*" >> "${MUXY_NETWORK_SENTINEL:?}"
exit 97
NETWORK_SENTINEL
    chmod 0755 "$executable"
}

prepare_shell_fixture() {
    local case_name="$1"
    local root="$2"
    local repository="$root/repository"
    local remote="$root/remote.git"
    local bin="$root/bin"
    local provider_log="$root/provider.log"
    local gh_log="$root/gh.log"
    local network_log="$root/network.log"
    reset_safe_path "$root"
    mkdir -p "$repository" "$bin"
    git -C "$repository" init -q -b main
    printf '%s\n' "$case_name" > "$repository/README.md"
    git -C "$repository" add README.md
    git -C "$repository" -c user.name=MuxyTests -c user.email=muxy-tests@example.invalid \
        commit -q -m initial
    git init -q --bare "$remote"
    git -C "$repository" remote add origin "$remote"
    git -C "$repository" push -q -u origin main
    write_fake_provider "$bin/codex"
    write_fake_gh "$bin/gh"
    write_network_sentinel "$bin/curl"
    write_network_sentinel "$bin/ssh"
    : > "$network_log"
    (
        cd "$repository"
        MUXY_FAKE_LOG="$provider_log" \
            MUXY_FIXTURE_ID="$case_name" \
            MUXY_NETWORK_SENTINEL="$network_log" \
            PATH="$bin:/usr/bin:/bin" \
            "$bin/codex" exec --ephemeral --sandbox read-only SYNTHETIC_PRIVATE_PROMPT \
            </dev/null > "$root/provider.out"
        MUXY_FAKE_LOG="$gh_log" \
            MUXY_FIXTURE_ID="$case_name" \
            MUXY_NETWORK_SENTINEL="$network_log" \
            PATH="$bin:/usr/bin:/bin" \
            "$bin/gh" pr view --repo example.invalid/owner/repository \
            </dev/null > "$root/gh.out"
    )
    grep -Fxq 'stdin_mode=closed' "$provider_log" || fail "provider stdin mode was not recorded"
    grep -Fxq 'stdin_length=0' "$provider_log" || fail "provider stdin length was not recorded"
    grep -Fxq 'prompt_length=24' "$provider_log" || fail "provider prompt length was not recorded"
    if grep -Fq 'SYNTHETIC_PRIVATE_PROMPT' "$provider_log"; then
        fail "provider log exposed prompt content"
    fi
    grep -Fxq "env:MUXY_FIXTURE_ID=$case_name" "$provider_log" || {
        fail "provider environment allowlist was not recorded"
    }
    grep -Fxq 'argv[0]=pr' "$gh_log" || fail "gh argv was not recorded"
    [[ ! -s "$network_log" ]] || fail "fixture attempted a network command"
    [[ "$(git -C "$repository" status --porcelain)" == "" ]] || fail "fixture repository is dirty"
    [[ "$(git -C "$repository" rev-parse HEAD)" == \
        "$(git --git-dir="$remote" rev-parse refs/heads/main)" ]] || {
        fail "fixture local and remote identities differ"
    }
}

verify_timeout_cleanup() {
    (
        cd "$PROJECT_ROOT"
        CARGO_NET_OFFLINE=true cargo test -p muxy-api --locked --offline \
            subprocess_timeout_terminates_the_group_and_reaps_descendants
    )
}

validate_fixture_case() {
    case "$1" in
        environment|repository|gh|providers|concurrency|full) ;;
        *) return 1 ;;
    esac
}

validate_staged_case() {
    case "$1" in
        identity|branch|changes|pr|ai|full) ;;
        *) return 1 ;;
    esac
}

self_test() {
    local root="$VERIFICATION_ROOT/self-test"
    local outside="$PROJECT_ROOT/target/p4-unsafe"
    reset_safe_path "$root"
    path_is_safe "$root/safe" || fail "safe containment path was rejected"
    if path_is_safe "$outside"; then
        fail "containment accepted a path outside the verification root"
    fi
    mkdir -p "$root/linked-target"
    ln -s "$root/linked-target" "$root/linked"
    if path_is_safe "$root/linked/child"; then
        fail "containment accepted a symlinked ancestor"
    fi
    rm -f "$root/linked"
    prepare_shell_fixture self-test "$root/fixture"
    if MUXY_NETWORK_SENTINEL="$root/fixture/network.log" \
        "$root/fixture/bin/curl" https://example.invalid > "$root/network-probe.out" 2>&1; then
        fail "network sentinel command unexpectedly succeeded"
    fi
    grep -Fq 'curl https://example.invalid' "$root/fixture/network.log" || {
        fail "network sentinel did not record the blocked command"
    }
    : > "$root/fixture/network.log"
    verify_timeout_cleanup
    if "$0" --fixture invalid > "$root/invalid-fixture.out" 2>&1; then
        fail "invalid fixture case was accepted"
    fi
    if "$0" --staged invalid full > "$root/invalid-profile.out" 2>&1; then
        fail "invalid staged profile was accepted"
    fi
    if "$0" --staged debug invalid > "$root/invalid-staged.out" 2>&1; then
        fail "invalid staged case was accepted"
    fi
    reset_safe_path "$root/cleanup-proof"
    rm -rf "$root/cleanup-proof"
    [[ ! -e "$root/cleanup-proof" ]] || fail "owned cleanup did not remove its target"
    rm -rf "$root"
    printf 'P4 VCS verifier self-test passed\n'
}

run_fixture() {
    local case_name="$1"
    local root="$VERIFICATION_ROOT/fixtures/$case_name"
    prepare_shell_fixture "$case_name" "$root"
    cd "$PROJECT_ROOT"
    export CARGO_NET_OFFLINE=true
    export MUXY_P4_FIXTURE_ROOT="$root"
    export MUXY_FAKE_LOG="$root/cargo-provider.log"
    export MUXY_FIXTURE_ID="$case_name"
    export MUXY_NETWORK_SENTINEL="$root/network.log"
    export PATH="$root/bin:$PATH"
    case "$case_name" in
        environment)
            cargo test -p muxy-api --locked --offline execution_environment
            ;;
        repository)
            cargo test -p muxy-api --locked --offline repository
            ;;
        gh)
            cargo test -p muxy-api --locked --offline repository::github
            ;;
        providers)
            cargo test -p muxy-core --locked --offline repository_ai
            cargo test -p muxy-api --locked --offline repository::ai
            ;;
        concurrency)
            cargo test -p muxy --locked --offline project_operations
            cargo test -p muxy --locked --offline repository::coordinator
            ;;
        full)
            cargo test -p muxy-api -p muxy-core -p muxy --locked --offline
            ;;
    esac
    printf 'P4 headless fixture passed: %s\n' "$case_name"
}

run_staged_headless_case() {
    local case_name="$1"
    case "$case_name" in
        identity)
            cargo test -p muxy-api --locked --offline repository_identity
            ;;
        branch)
            cargo test -p muxy-api --locked --offline repository_mutate_branches
            cargo test -p muxy --locked --offline repository_picker
            ;;
        changes)
            cargo test -p muxy-api --locked --offline repository_mutate_stages
            cargo test -p muxy --locked --offline changes
            ;;
        pr)
            cargo test -p muxy-api --locked --offline repository::github
            cargo test -p muxy --locked --offline pull_request
            ;;
        ai)
            cargo test -p muxy-api --locked --offline repository::ai
            cargo test -p muxy --locked --offline repository_ai
            ;;
        full)
            cargo test -p muxy-api -p muxy-core -p muxy --locked --offline
            ;;
    esac
}

print_manual_checklist() {
    local profile="$1"
    local case_name="$2"
    local app="$3"
    local fixture="$4"
    printf 'Prepared signed P4 %s fixture: %s\n' "$profile" "$app"
    printf 'Fixture root: %s\n' "$fixture"
    printf 'Repository: %s\n' "$fixture/repository"
    printf 'Manual case: %s\n' "$case_name"
    printf 'Manual launch command, intentionally not run by this verifier:\n'
    printf 'env HOME=%q CFFIXED_USER_HOME=%q TMPDIR=%q XDG_CONFIG_HOME=%q MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY=%q %q\n' \
        "$fixture/home" \
        "$fixture/home" \
        "$fixture/home/tmp" \
        "$fixture/xdg-config" \
        "$fixture/app-support" \
        "$app/Contents/MacOS/MuxyTests"
    printf '%s\n' \
        'Verify the selected project/worktree alone controls repository identity.' \
        'Verify branch, changes, pull-request, and AI popovers replace each other and restore focus.' \
        'Verify compact row grids, padding, hover actions, draggable scrollbars, and no flashing labels.' \
        'Verify every mutation disables competing P3/P4 actions and refreshes only affected truth.' \
        'Verify provider selection, project prompt set/clear, confirmation context drift, cancellation, and partial-success copy.' \
        'Close the app and confirm the fixture socket and process are gone before deleting the fixture.'
}

run_staged() {
    local profile="$1"
    local case_name="$2"
    local source_app="$PROJECT_ROOT/target/$profile/Muxy.app"
    local label="p4-$profile-$case_name"
    local fixture="$VERIFICATION_ROOT/staged/$profile/$case_name"
    local staged_app staged_cli
    prepare_shell_fixture "$case_name" "$fixture"
    mkdir -p "$fixture/home/tmp" "$fixture/xdg-config" "$fixture/app-support"
    export CARGO_NET_OFFLINE=true
    export MUXY_P4_FIXTURE_ROOT="$fixture"
    export MUXY_FAKE_LOG="$fixture/cargo-provider.log"
    export MUXY_FIXTURE_ID="$case_name"
    export MUXY_NETWORK_SENTINEL="$fixture/network.log"
    export PATH="$fixture/bin:$PATH"
    run_staged_headless_case "$case_name"
    "$SCRIPT_DIR/build-app.sh" "$profile"
    "$SCRIPT_DIR/verify-bundle.sh" "$source_app" "$profile"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "$label")"
    staged_cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    codesign --verify --deep --strict "$staged_app"
    cmp -s "$SOURCE_CLI" "$staged_cli" || fail "staged CLI differs from retained source"
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$staged_app/Contents/Info.plist")" == \
        "com.muxy.tests" ]] || fail "staged bundle identity differs"
    [[ ! -S "$fixture/muxy.sock" && ! -S "$fixture/muxy-dev.sock" ]] || {
        fail "staged fixture has a live socket before manual testing"
    }
    print_manual_checklist "$profile" "$case_name" "$staged_app" "$fixture"
}

for command_name in bash cargo chmod cmp git grep; do
    require_command "$command_name"
done

case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "--self-test accepts no additional arguments"
        self_test
        ;;
    --fixture)
        (($# == 2)) || fail "usage: scripts/verify-p4-vcs.sh --fixture <environment|repository|gh|providers|concurrency|full>"
        validate_fixture_case "$2" || fail "unknown fixture case: $2"
        run_fixture "$2"
        ;;
    --staged)
        (($# == 3)) || fail "usage: scripts/verify-p4-vcs.sh --staged <debug|release> <identity|branch|changes|pr|ai|full>"
        [[ "$2" == debug || "$2" == release ]] || fail "unknown staged profile: $2"
        validate_staged_case "$3" || fail "unknown staged case: $3"
        require_command codesign
        require_command plutil
        run_staged "$2" "$3"
        ;;
    *)
        fail "usage: scripts/verify-p4-vcs.sh --self-test | --fixture <case> | --staged <profile> <case>"
        ;;
esac
