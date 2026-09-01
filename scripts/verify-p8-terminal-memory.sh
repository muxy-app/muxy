#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_PARENT="$PROJECT_ROOT/target/test-verification"
readonly VERIFICATION_ROOT="$VERIFICATION_PARENT/p8-terminal-memory"
readonly ROOT_MARKER="$VERIFICATION_ROOT/.muxy-p8-verifier"
readonly ROOT_OWNER="muxy-p8-terminal-memory-verifier-v1"
readonly CASE_MARKER=".muxy-p8-case"
readonly CASE_OWNER="muxy-p8-terminal-memory-case-v1"
readonly STAGED_APPS_ROOT="$VERIFICATION_PARENT/apps"
readonly PRODUCTION_DEBUG_PROFILE="$HOME/.muxy-dev"
readonly PRODUCTION_RELEASE_PROFILE="$HOME/.muxy"
readonly LEGACY_SWIFT_PROFILE="$HOME/Library/Application Support/Muxy"
readonly LEGACY_VOLATILE_PROFILER="$LEGACY_SWIFT_PROFILE/Diagnostics/profiler.jsonl"
TRACKED_PIDS=(0)

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

root_ancestors_are_safe() {
    local path
    for path in "$PROJECT_ROOT/target" "$VERIFICATION_PARENT" "$VERIFICATION_ROOT"; do
        [[ ! -L "$path" ]] || return 1
        [[ ! -e "$path" || -d "$path" ]] || return 1
    done
}

root_is_owned() {
    [[ -d "$VERIFICATION_ROOT" && ! -L "$VERIFICATION_ROOT" ]] || return 1
    [[ -f "$ROOT_MARKER" && ! -L "$ROOT_MARKER" ]] || return 1
    [[ "$(<"$ROOT_MARKER")" == "$ROOT_OWNER" ]]
}

prepare_root() {
    root_ancestors_are_safe || fail "verification root has an unsafe ancestor"
    mkdir -p "$VERIFICATION_PARENT"
    if [[ -e "$VERIFICATION_ROOT" ]]; then
        root_is_owned || fail "verification root is not owned by the P8 verifier"
    else
        mkdir "$VERIFICATION_ROOT"
        printf '%s\n' "$ROOT_OWNER" > "$ROOT_MARKER"
        chmod 0700 "$VERIFICATION_ROOT"
        chmod 0600 "$ROOT_MARKER"
    fi
    root_ancestors_are_safe || fail "verification root changed while preparing it"
}

path_is_safe() {
    local path="$1" relative cursor component old_ifs
    local -a components
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

case_is_owned() {
    local path="$1" marker="$1/$CASE_MARKER"
    [[ -d "$path" && ! -L "$path" ]] || return 1
    [[ -f "$marker" && ! -L "$marker" ]] || return 1
    [[ "$(<"$marker")" == "$CASE_OWNER" ]]
}

prepare_case() {
    local path="$1"
    prepare_root
    path_is_safe "$path" || fail "refusing unsafe P8 case path: $path"
    if [[ -e "$path" ]]; then
        case_is_owned "$path" || fail "P8 case path is not verifier-owned: $path"
        rm -rf -- "$path"
    fi
    mkdir -p "$path"
    printf '%s\n' "$CASE_OWNER" > "$path/$CASE_MARKER"
    chmod 0600 "$path/$CASE_MARKER"
}

reject_production_target() {
    local path="$1"
    [[ -n "$path" && "$path" == /* ]] || return 1
    case "$path" in
        "$HOME"|"$PRODUCTION_DEBUG_PROFILE"|"$PRODUCTION_DEBUG_PROFILE"/*|\
        "$PRODUCTION_RELEASE_PROFILE"|"$PRODUCTION_RELEASE_PROFILE"/*|\
        "$LEGACY_SWIFT_PROFILE"|"$LEGACY_SWIFT_PROFILE"/*)
            return 1
            ;;
    esac
}

require_injected_root() {
    local path="${MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY:-}"
    [[ -n "$path" ]] || return 1
    reject_production_target "$path" || return 1
    path_is_safe "$path" || return 1
    [[ -d "$path" && ! -L "$path" ]]
}

pid_is_tracked() {
    local candidate="$1" held
    for held in "${TRACKED_PIDS[@]}"; do
        [[ "$held" == 0 ]] && continue
        [[ "$held" == "$candidate" ]] && return 0
    done
    return 1
}

pid_descends_from() {
    local candidate="$1" ancestor="$2" parent
    while [[ "$candidate" =~ ^[1-9][0-9]*$ ]] && ((candidate > 1)); do
        [[ "$candidate" == "$ancestor" ]] && return 0
        parent="$(ps -o ppid= -p "$candidate" 2>/dev/null | tr -d '[:space:]')"
        [[ "$parent" =~ ^[1-9][0-9]*$ && "$parent" != "$candidate" ]] || return 1
        candidate="$parent"
    done
    return 1
}

track_pid() {
    local pid="$1" root="${2:-$$}"
    [[ "$pid" =~ ^[1-9][0-9]*$ ]] || fail "invalid verifier pid: $pid"
    kill -0 "$pid" 2>/dev/null || return 0
    if ! pid_descends_from "$pid" "$root"; then
        kill -0 "$pid" 2>/dev/null || return 0
        fail "refusing unowned pid: $pid"
    fi
    pid_is_tracked "$pid" || TRACKED_PIDS+=("$pid")
}

track_descendants() {
    local root="$1" current child
    local -a queue
    queue=("$root")
    while ((${#queue[@]} > 0)); do
        current="${queue[0]}"
        queue=("${queue[@]:1}")
        while IFS= read -r child; do
            [[ "$child" =~ ^[1-9][0-9]*$ ]] || continue
            track_pid "$child" "$root"
            queue+=("$child")
        done < <(pgrep -P "$current" 2>/dev/null || true)
    done
}

cleanup_tracked() {
    local pid attempt alive
    for pid in "${TRACKED_PIDS[@]}"; do
        [[ "$pid" == 0 ]] && continue
        kill -0 "$pid" 2>/dev/null || continue
        kill -TERM "$pid" 2>/dev/null || true
    done
    attempt=0
    while ((attempt < 100)); do
        alive=false
        for pid in "${TRACKED_PIDS[@]}"; do
            [[ "$pid" == 0 ]] && continue
            if kill -0 "$pid" 2>/dev/null; then
                alive=true
                break
            fi
        done
        [[ "$alive" == false ]] && break
        sleep 0.05
        ((attempt += 1))
    done
    for pid in "${TRACKED_PIDS[@]}"; do
        [[ "$pid" == 0 ]] && continue
        if kill -0 "$pid" 2>/dev/null; then
            kill -KILL "$pid" 2>/dev/null || true
        fi
        wait "$pid" 2>/dev/null || true
    done
    TRACKED_PIDS=(0)
}

assert_tracked_stopped() {
    local pid attempt alive
    attempt=0
    while ((attempt < 100)); do
        alive=false
        for pid in "${TRACKED_PIDS[@]}"; do
            [[ "$pid" == 0 ]] && continue
            if kill -0 "$pid" 2>/dev/null; then
                alive=true
                break
            fi
        done
        [[ "$alive" == false ]] && return 0
        sleep 0.05
        ((attempt += 1))
    done
    for pid in "${TRACKED_PIDS[@]}"; do
        [[ "$pid" == 0 ]] && continue
        kill -0 "$pid" 2>/dev/null && printf 'surviving pid: %s\n' "$pid" >&2
    done
    fail "staged app descendants survived normal close"
}

wait_for_path() {
    local path="$1" kind="$2" attempts="$3" attempt
    attempt=0
    while ((attempt < attempts)); do
        case "$kind" in
            file) [[ -f "$path" ]] && return 0 ;;
            socket) [[ -S "$path" ]] && return 0 ;;
            missing) [[ ! -e "$path" && ! -L "$path" ]] && return 0 ;;
            *) return 1 ;;
        esac
        sleep 0.05
        ((attempt += 1))
    done
    return 1
}

snapshot_path() {
    local path="$1" destination="$2" excluded="${3:-}" entry relative kind detail
    if [[ ! -e "$path" && ! -L "$path" ]]; then
        printf 'missing\n' > "$destination"
        return
    fi
    {
        while IFS= read -r -d '' entry; do
            [[ -n "$excluded" && "$entry" == "$excluded" ]] && continue
            relative="${entry#"$path"}"
            [[ -n "$relative" ]] || relative="."
            if [[ -L "$entry" ]]; then
                kind="symlink"
                detail="$(readlink "$entry")"
            elif [[ -f "$entry" ]]; then
                kind="file"
                detail="$(shasum -a 256 "$entry" | cut -d ' ' -f 1)"
            elif [[ -d "$entry" ]]; then
                kind="directory"
                detail="-"
            elif [[ -S "$entry" ]]; then
                kind="socket"
                detail="-"
            else
                kind="other"
                detail="-"
            fi
            printf '%s|%s|%s|%s\n' "$kind" "$(stat -f '%Lp:%u:%g:%i:%m:%z' "$entry")" "$detail" "$relative"
        done < <(find "$path" -print0 | sort -z)
    } > "$destination"
}

snapshot_production() {
    local destination="$1"
    mkdir -p "$destination"
    snapshot_path "$PRODUCTION_DEBUG_PROFILE" "$destination/debug-profile"
    snapshot_path "$PRODUCTION_RELEASE_PROFILE" "$destination/release-profile"
    snapshot_path "$LEGACY_SWIFT_PROFILE" "$destination/legacy-profile" "$LEGACY_VOLATILE_PROFILER"
}

compare_snapshots() {
    local before="$1" after="$2" name
    for name in debug-profile release-profile legacy-profile; do
        cmp -s "$before/$name" "$after/$name" || fail "production state changed: $name"
    done
}

scope_checks() {
    local paths resource_key_matches
    paths=(
        PLAN.md
        ARCHITECTURE.md
        crates/muxy/src
        crates/muxy-core/src
        crates/muxy-terminal/src
        crates/muxy-proto/src
        crates/ghostty-host/src
        Muxy/Resources
        docs
    )
    if rg -n -i -g '!*.swift' \
        'resource usage|resource monitor|CPU and memory usage|proc_pid_rusage|Send to Background|sessions? popover|background[- ]session popover|background terminal sessions|detached session|session adoption|adopt(ing|ed)? (a )?session|list-sessions|kill-session|sessions\.(list|kill)|worktree\.offline|Swift daemon|cross-attach|1\.x framed protocol|SessionFrame' \
        "${paths[@]}"; then
        fail "rejected P8 surface remains"
    fi
    resource_key_matches="$(rg -n -F 'muxy.showResourceUsageInStatusBar' \
        crates Muxy/Resources docs PLAN.md ARCHITECTURE.md || true)"
    [[ "$(printf '%s\n' "$resource_key_matches" | sed '/^$/d' | wc -l | tr -d '[:space:]')" == 3 ]] || {
        printf '%s\n' "$resource_key_matches"
        fail "removed resource setting escaped its preservation test"
    }
    [[ "$(printf '%s\n' "$resource_key_matches" | cut -d: -f1 | sort -u)" == \
        "crates/muxy-core/src/prefs/settings.rs" ]] || fail "removed resource setting remains in production surface"
    [[ ! -e "$PROJECT_ROOT/crates/muxy-session" ]] || fail "Phase 1 must not add muxy-session"
    rg -q 'pub mod session;' crates/muxy-proto/src/lib.rs || fail "private session protocol is not exported"
    rg -q -F '*b"MXS2"' crates/muxy-proto/src/session/codec.rs || fail "MXS2 protocol magic is missing"
    rg -q 'pub mod offline;' crates/muxy-terminal/src/lib.rs || fail "portable offline policy is not exported"
    rg -q 'ghostty_surface_set_data_callback' crates/ghostty-host/src/surface.rs || fail "Ghostty data callback seam is missing"
    rg -q 'is_alternate_screen' crates/ghostty-host/src/surface.rs || fail "Ghostty alternate-screen seam is missing"
    printf 'P8 scope checks passed\n'
}

validate_staged_app() {
    local app="$1" plist executable marker parent
    [[ "$app" == /* && -d "$app" && ! -L "$app" ]] || fail "staged app path is invalid"
    [[ "$app" == "$STAGED_APPS_ROOT/"*/MuxyTests.app ]] || fail "staged app is outside the staging root"
    parent="$(dirname "$app")"
    marker="$parent/.muxy-stage-owner"
    [[ -f "$marker" && ! -L "$marker" && "$(<"$marker")" == "$app" ]] || fail "staged app ownership is invalid"
    plist="$app/Contents/Info.plist"
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$plist")" == "com.muxy.tests" ]] || fail "staged bundle identity differs"
    [[ "$(plutil -extract CFBundleExecutable raw -o - "$plist")" == "MuxyTests" ]] || fail "staged executable name differs"
    executable="$app/Contents/MacOS/MuxyTests"
    [[ -x "$executable" ]] || fail "staged executable is missing"
    codesign --verify --deep --strict "$app"
}

run_launch_close() {
    local mode="$1" source_app staged_app case_root case_code app_support socket_name socket executable cli log app_pid status
    local before after
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-launch-close")"
    validate_staged_app "$staged_app"
    case_code="d"
    [[ "$mode" == release ]] && case_code="r"
    case_root="$VERIFICATION_ROOT/$case_code"
    prepare_case "$case_root"
    app_support="$case_root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" require_injected_root || fail "injected root was rejected"
    socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    socket="$app_support/$socket_name"
    ((${#socket} < 104)) || fail "staged socket path exceeds the macOS limit"
    executable="$staged_app/Contents/MacOS/MuxyTests"
    cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    log="$case_root/app.log"
    before="$case_root/production-before"
    after="$case_root/production-after"
    snapshot_production "$before"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid=$!
    track_pid "$app_pid"
    trap cleanup_tracked EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    wait_for_path "$socket" socket 600 || {
        sed -n '1,240p' "$log"
        fail "staged app did not become ready"
    }
    [[ "$(stat -f '%Lp' "$socket")" == 600 ]] || fail "staged socket mode is not 0600"
    MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=5 "$cli" list-projects > "$case_root/list-projects.txt"
    track_descendants "$app_pid"
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in {1..600}; do
        track_descendants "$app_pid"
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$app_pid" 2>/dev/null && fail "staged app did not close normally"
    set +e
    wait "$app_pid"
    status=$?
    set -e
    [[ "$status" == 0 ]] || {
        sed -n '1,240p' "$log"
        fail "staged app exited with status $status"
    }
    wait_for_path "$socket" missing 100 || fail "staged socket remained after close"
    wait_for_path "$app_support/.muxy-test-close-main-window" missing 100 || fail "close request remained after close"
    assert_tracked_stopped
    cleanup_tracked
    trap - EXIT INT TERM
    snapshot_production "$after"
    compare_snapshots "$before" "$after"
    printf 'P8 staged %s launch-close passed\n' "$mode"
}

self_test() {
    local case_root injected signal sleeper outside linked before after changed volatile stable
    case_root="$VERIFICATION_ROOT/self-test"
    prepare_case "$case_root"
    trap cleanup_tracked EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    path_is_safe "$case_root" || fail "self-test rejected its owned case"
    reject_production_target "$PRODUCTION_DEBUG_PROFILE" && fail "self-test accepted the debug profile"
    reject_production_target "$PRODUCTION_RELEASE_PROFILE/settings.json" && fail "self-test accepted the release profile"
    reject_production_target "$LEGACY_SWIFT_PROFILE" && fail "self-test accepted the legacy profile"
    if MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY='' require_injected_root; then
        fail "self-test accepted a missing injected root"
    fi
    injected="$case_root/injected"
    mkdir "$injected"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$injected" require_injected_root || fail "self-test rejected an owned injected root"
    outside="$case_root/outside"
    mkdir "$outside"
    linked="$case_root/linked"
    ln -s "$outside" "$linked"
    path_is_safe "$linked" && fail "self-test accepted a symlinked path"
    rm -- "$linked"
    signal="$case_root/signal"
    (sleep 0.05; printf '%s\n' ready > "$signal") &
    sleeper=$!
    track_pid "$sleeper"
    wait_for_path "$signal" file 40 || fail "self-test bounded wait missed a file"
    wait "$sleeper"
    TRACKED_PIDS=(0)
    wait_for_path "$case_root/missing" file 2 && fail "self-test bounded wait did not time out"
    /bin/sleep 30 &
    sleeper=$!
    track_pid "$sleeper"
    pid_is_tracked "$sleeper" || fail "self-test pid was not tracked"
    cleanup_tracked
    kill -0 "$sleeper" 2>/dev/null && fail "self-test tracked pid survived cleanup"
    before="$case_root/snapshot-before"
    after="$case_root/snapshot-after"
    changed="$case_root/snapshot-changed"
    volatile="$injected/volatile"
    stable="$injected/stable"
    printf '%s\n' first > "$volatile"
    printf '%s\n' fixed > "$stable"
    snapshot_path "$injected" "$before" "$volatile"
    printf '%s\n' second > "$volatile"
    snapshot_path "$injected" "$after" "$volatile"
    cmp -s "$before" "$after" || fail "self-test exact snapshot exclusion differed"
    printf '%s\n' changed > "$stable"
    snapshot_path "$injected" "$changed" "$volatile"
    cmp -s "$before" "$changed" && fail "self-test snapshot exclusion hid another file"
    snapshot_path "$injected" "$before"
    snapshot_path "$injected" "$after"
    cmp -s "$before" "$after" || fail "self-test stable snapshot differed"
    trap - EXIT INT TERM
    printf 'P8 terminal memory self-test passed\n'
}

for command_name in cmp cut find pgrep ps readlink rg sed shasum sort stat tr wc; do
    require_command "$command_name"
done

case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p8-terminal-memory.sh --self-test"
        self_test
        ;;
    --fixture)
        (($# == 2)) || fail "usage: scripts/verify-p8-terminal-memory.sh --fixture CASE"
        [[ "$2" == scope ]] || fail "unsupported Phase 1 fixture: $2"
        scope_checks
        ;;
    --mode)
        (($# == 4)) || fail "usage: scripts/verify-p8-terminal-memory.sh --mode PROFILE --case CASE"
        [[ "$2" == debug || "$2" == release ]] || fail "profile must be debug or release"
        [[ "$3" == --case && "$4" == launch-close ]] || fail "unsupported Phase 1 staged case"
        for command_name in codesign plutil; do
            require_command "$command_name"
        done
        run_launch_close "$2"
        ;;
    *)
        fail "usage: scripts/verify-p8-terminal-memory.sh --self-test | --fixture scope | --mode PROFILE --case launch-close"
        ;;
esac
