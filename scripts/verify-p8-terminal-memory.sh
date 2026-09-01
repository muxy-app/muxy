#!/usr/bin/env bash
set -Eeuo pipefail
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

unexpected_failure() {
    local status="$1" line="$2"
    fail "unexpected verifier command failure at line $line (status $status)"
}

trap 'unexpected_failure "$?" "$LINENO"' ERR

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

track_session_daemon() {
    local pid="$1" executable="$2" socket="$3" command
    [[ "$pid" =~ ^[1-9][0-9]*$ ]] || fail "invalid session daemon pid: $pid"
    command="$(ps -o command= -p "$pid" 2>/dev/null)"
    [[ "$command" == "$executable daemon "*"--socket $socket"* ]] || \
        fail "refusing an unowned session daemon: $pid"
    pid_is_tracked "$pid" || TRACKED_PIDS+=("$pid")
}

track_relaunched_app() {
    local pid="$1" executable="$2" command
    [[ "$pid" =~ ^[1-9][0-9]*$ ]] || fail "invalid relaunched app pid: $pid"
    command="$(ps -o command= -p "$pid" 2>/dev/null)"
    [[ "$command" == "$executable" ]] || fail "refusing an unowned relaunched app: $pid"
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
    [[ -d "$PROJECT_ROOT/crates/muxy-session" ]] || fail "Phase 2 session crate is missing"
    rg -q 'pub mod session;' crates/muxy-proto/src/lib.rs || fail "private session protocol is not exported"
    rg -q -F '*b"MXS2"' crates/muxy-proto/src/session/codec.rs || fail "MXS2 protocol magic is missing"
    rg -q 'pub mod offline;' crates/muxy-terminal/src/lib.rs || fail "portable offline policy is not exported"
    rg -q 'ghostty_surface_set_data_callback' crates/ghostty-host/src/surface.rs || fail "Ghostty data callback seam is missing"
    rg -q 'is_alternate_screen' crates/ghostty-host/src/surface.rs || fail "Ghostty alternate-screen seam is missing"
    printf 'P8 scope checks passed\n'
}

validate_staged_app() {
    local app="$1" plist executable session_executable marker parent
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
    session_executable="$app/Contents/MacOS/muxy-session-v2"
    [[ -x "$session_executable" ]] || fail "staged session executable is missing"
    [[ "$(stat -f '%Lp' "$session_executable")" == 755 ]] || fail "staged session executable mode is not 0755"
    codesign --verify --strict "$session_executable"
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
    status=0
    wait "$app_pid" || status=$?
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

run_daemon_harness() {
    local mode="$1" source_app staged_app case_root case_code session_executable resources
    local before after
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-daemon-harness")"
    validate_staged_app "$staged_app"
    case_code="sd"
    [[ "$mode" == release ]] && case_code="sr"
    case_root="$VERIFICATION_ROOT/$case_code"
    prepare_case "$case_root"
    session_executable="$staged_app/Contents/MacOS/muxy-session-v2"
    resources="$staged_app/Contents/Resources"
    before="$case_root/production-before"
    after="$case_root/production-after"
    snapshot_production "$before"
    MUXY_TEST_SESSION_BINARY="$session_executable" \
        MUXY_TEST_SESSION_RESOURCES="$resources" \
        cargo test -p muxy-session --test session_process --locked --offline \
        daemon_replays_detached_output_replaces_clients_resizes_and_exits_idle -- \
        --exact --nocapture
    snapshot_production "$after"
    compare_snapshots "$before" "$after"
    printf 'P8 staged %s daemon harness passed\n' "$mode"
}

persistent_contracts() {
    cargo test -p muxy-core workspace --locked --offline
    cargo test -p muxy terminal::session --locked --offline
    cargo test -p muxy migration --locked --offline
    printf 'P8 persistent contracts passed\n'
}

write_persistent_fixture() {
    local app_support="$1" project_path="$1/project"
    local project_id="AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"
    local worktree_id="11111111-2222-4333-8444-555555555555"
    local pane_id="22222222-3333-4444-8555-666666666666"
    local area_id="77777777-8888-4999-8AAA-BBBBBBBBBBBB"
    mkdir -p "$project_path" "$app_support/worktrees"
    cat > "$app_support/projects.json" <<JSON
[
  {
    "id": "$project_id",
    "name": "P8 Recovery",
    "path": "$project_path",
    "sortOrder": 0,
    "createdAt": 1,
    "worktreesEnabled": false,
    "isPinned": false
  }
]
JSON
    cat > "$app_support/worktrees/$project_id.json" <<JSON
[
  {
    "id": "$worktree_id",
    "name": "P8 Recovery",
    "path": "$project_path",
    "source": "muxy",
    "isPrimary": true,
    "createdAt": 1
  }
]
JSON
    cat > "$app_support/preferences.json" <<JSON
{
  "muxy.activeProjectID": "$project_id",
  "muxy.activeWorktreeIDs": {
    "$project_id": "$worktree_id"
  },
  "muxy.terminalPersistentSession.enabled": true
}
JSON
    cat > "$app_support/workspaces.json" <<JSON
[
  {
    "projectID": "$project_id",
    "worktreeID": "$worktree_id",
    "worktreePath": "$project_path",
    "focusedAreaID": "$area_id",
    "topLevelTabOrder": ["$pane_id"],
    "topLevelTabLayout": {
      "type": "group",
      "group": {
        "tabIDs": ["$pane_id"],
        "activeTabID": "$pane_id"
      }
    },
    "root": {
      "type": "tabArea",
      "tabArea": {
        "id": "$area_id",
        "projectPath": "$project_path",
        "tabs": [
          {
            "kind": "terminal",
            "id": "$pane_id",
            "isPinned": false,
            "projectPath": "$project_path"
          }
        ],
        "activeTabIndex": 0
      }
    }
  }
]
JSON
    chmod 0600 "$app_support/projects.json" "$app_support/worktrees/$project_id.json" \
        "$app_support/preferences.json" "$app_support/workspaces.json"
}

wait_for_cli() {
    local cli="$1" socket="$2" output="$3" attempt
    attempt=0
    while ((attempt < 200)); do
        if MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=1 "$cli" list-panes > "$output" 2>/dev/null; then
            return 0
        fi
        sleep 0.05
        ((attempt += 1))
    done
    return 1
}

wait_for_screen_text() {
    local cli="$1" socket="$2" pane_id="$3" pattern="$4" output="$5" attempt
    attempt=0
    while ((attempt < 200)); do
        if MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=2 \
            "$cli" read-screen --pane "$pane_id" --lines 200 > "$output" 2>/dev/null && \
            rg -q "$pattern" "$output"; then
            return 0
        fi
        sleep 0.05
        ((attempt += 1))
    done
    return 1
}

close_pane_checked() {
    local cli="$1" socket="$2" pane_id="$3" reply
    reply="$(MUXY_SOCKET_PATH="$socket" "$cli" close-pane --pane "$pane_id")" || \
        fail "tab close command failed"
    [[ "$reply" != error:* ]] || fail "tab close command failed: $reply"
    printf '%s\n' "$reply"
}

session_shell_for_directory() {
    local daemon_pid="$1" directory="$2" child cwd
    while IFS= read -r child; do
        [[ "$child" =~ ^[1-9][0-9]*$ ]] || continue
        cwd="$(lsof -a -p "$child" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p')"
        if [[ "$cwd" == "$directory" ]]; then
            printf '%s\n' "$child"
            return 0
        fi
    done < <(pgrep -P "$daemon_pid" 2>/dev/null || true)
    return 1
}

prepare_session_root() {
    local root
    root="$(mktemp -d "/private/tmp/muxy-p8-$PPID.XXXXXX")"
    [[ "$root" == /private/tmp/muxy-p8-"$PPID".* && -d "$root" && ! -L "$root" ]] || \
        fail "temporary session root is unsafe"
    printf '%s\n' "$CASE_OWNER" > "$root/$CASE_MARKER"
    chmod 0700 "$root"
    chmod 0600 "$root/$CASE_MARKER"
    printf '%s\n' "$root"
}

remove_session_root() {
    local root="$1"
    [[ "$root" == /private/tmp/muxy-p8-"$PPID".* && -d "$root" && ! -L "$root" ]] || \
        fail "refusing unsafe temporary session root cleanup"
    [[ -f "$root/$CASE_MARKER" && "$(<"$root/$CASE_MARKER")" == "$CASE_OWNER" ]] || \
        fail "temporary session root is not verifier-owned"
    rm -rf -- "$root"
}

run_session_recovery() {
    local mode="$1" termination="$2" source_app staged_app case_root app_support socket session_root session_socket
    local executable cli pane_id app_pid app_pid_two daemon_pid shell_pid status before after log close_reply
    pane_id="22222222-3333-4444-8555-666666666666"
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-$termination-recovery")"
    validate_staged_app "$staged_app"
    case_root="$VERIFICATION_ROOT/cr"
    [[ "$termination" == normal ]] && case_root="$VERIFICATION_ROOT/nr"
    if [[ "$mode" == release ]]; then
        case_root="$VERIFICATION_ROOT/rr"
        [[ "$termination" == normal ]] && case_root="$VERIFICATION_ROOT/rnr"
    fi
    prepare_case "$case_root"
    app_support="$case_root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    write_persistent_fixture "$app_support"
    session_root="$(prepare_session_root)"
    session_socket="$session_root/control.sock"
    executable="$staged_app/Contents/MacOS/MuxyTests"
    cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    socket="$app_support/muxy-dev.sock"
    [[ "$mode" == release ]] && socket="$app_support/muxy.sock"
    before="$case_root/production-before"
    after="$case_root/production-after"
    snapshot_production "$before"
    log="$case_root/first.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P8_SESSION_SOCKET_PATH="$session_socket" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid=$!
    track_pid "$app_pid"
    trap cleanup_tracked EXIT
    wait_for_cli "$cli" "$socket" "$case_root/first-panes.txt" || {
        sed -n '1,240p' "$log"
        fail "persistent staged app did not become ready"
    }
    MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=3 \
        "$cli" read-screen --pane "$pane_id" --lines 20 > "$case_root/initial-screen.txt" || true
    wait_for_path "$session_socket" socket 200 || fail "persistent session socket was not created"
    for _ in {1..200}; do
        jq -e --arg pane "$pane_id" \
            '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
            "$app_support/workspaces.json" >/dev/null 2>&1 && break
        sleep 0.05
    done
    jq -e --arg pane "$pane_id" \
        '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
        "$app_support/workspaces.json" >/dev/null || fail "session establishment was not published"
    MUXY_SOCKET_PATH="$socket" "$cli" send --pane "$pane_id" \
        "i=0; while [ \$i -lt 120 ]; do echo P8_ABSENT_\$i; i=\$((i+1)); sleep 0.05; done"
    MUXY_SOCKET_PATH="$socket" "$cli" send-keys --pane "$pane_id" Enter
    wait_for_screen_text "$cli" "$socket" "$pane_id" 'P8_ABSENT_[0-9]+' \
        "$case_root/before-crash.txt" || fail "session command did not start"
    daemon_pid="$(ps -axo pid=,command= | awk -v socket="$session_socket" \
        '!found && $0 ~ /muxy-session-v2 daemon/ && $0 ~ /--socket/ && index($0, socket) { print $1; found = 1 }')"
    [[ "$daemon_pid" =~ ^[1-9][0-9]*$ ]] || fail "session daemon PID was not found"
    track_session_daemon "$daemon_pid" "$staged_app/Contents/MacOS/muxy-session-v2" "$session_socket"
    shell_pid="$(session_shell_for_directory "$daemon_pid" "$app_support/project" || true)"
    [[ "$shell_pid" =~ ^[1-9][0-9]*$ ]] || fail "session shell PID was not found"
    track_pid "$shell_pid" "$daemon_pid"
    if [[ "$termination" == crash ]]; then
        kill -KILL "$app_pid"
        status=0
        wait "$app_pid" || status=$?
        [[ "$status" != 0 ]] || fail "crash process exited normally"
    else
        printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
        for _ in {1..600}; do
            ! kill -0 "$app_pid" 2>/dev/null && break
            sleep 0.05
        done
        kill -0 "$app_pid" 2>/dev/null && fail "persistent app did not close normally"
        status=0
        wait "$app_pid" || status=$?
        [[ "$status" == 0 ]] || fail "persistent app exited with status $status"
    fi
    sleep 0.5
    kill -0 "$shell_pid" 2>/dev/null || fail "session shell died with the app"
    log="$case_root/second.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P8_SESSION_SOCKET_PATH="$session_socket" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid_two=$!
    track_pid "$app_pid_two"
    wait_for_cli "$cli" "$socket" "$case_root/second-panes.txt" || {
        sed -n '1,240p' "$log"
        fail "recovery app did not become ready"
    }
    rg -q "^$pane_id" "$case_root/second-panes.txt" || fail "recovered tab identity changed"
    wait_for_screen_text "$cli" "$socket" "$pane_id" 'P8_ABSENT_(1[0-9]|[2-9][0-9])' \
        "$case_root/after-crash.txt" || fail "absent-period output did not replay"
    MUXY_SOCKET_PATH="$socket" "$cli" send --pane "$pane_id" 'echo P8_POST_RECOVERY'
    MUXY_SOCKET_PATH="$socket" "$cli" send-keys --pane "$pane_id" Enter
    wait_for_screen_text "$cli" "$socket" "$pane_id" 'P8_POST_RECOVERY' \
        "$case_root/post-recovery.txt" || fail "post-recovery input/output failed"
    close_reply="$(close_pane_checked "$cli" "$socket" "$pane_id")"
    for _ in {1..200}; do
        ! kill -0 "$shell_pid" 2>/dev/null && break
        sleep 0.05
    done
    if kill -0 "$shell_pid" 2>/dev/null; then
        printf 'close reply: %s\n' "$close_reply" >&2
        ps -o pid=,ppid=,pgid=,sess=,state=,command= -p "$shell_pid" >&2 || true
        ps -o pid=,ppid=,pgid=,sess=,state=,command= -p "$daemon_pid" >&2 || true
        fail "tab close did not terminate the session shell"
    fi
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in {1..600}; do
        ! kill -0 "$app_pid_two" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$app_pid_two" 2>/dev/null && fail "recovery app did not close normally"
    status=0
    wait "$app_pid_two" || status=$?
    [[ "$status" == 0 ]] || fail "recovery app exited with status $status"
    cleanup_tracked
    trap - EXIT
    remove_session_root "$session_root"
    snapshot_production "$after"
    compare_snapshots "$before" "$after"
    printf 'P8 staged %s %s recovery passed\n' "$mode" "$termination"
}

run_crash_recovery() {
    run_session_recovery "$1" crash
}

run_normal_recovery() {
    run_session_recovery "$1" normal
}

mark_fixture_established() {
    local app_support="$1" output="$1/workspaces-established.json"
    jq 'walk(if type == "object" and .id? == "22222222-3333-4444-8555-666666666666" then .rustPersistentSession = true else . end)' \
        "$app_support/workspaces.json" > "$output"
    chmod 0600 "$output"
    mv "$output" "$app_support/workspaces.json"
}

run_recovery_state_case() {
    local mode="$1" state_name="$2" staged_app="$3" case_root="$4" before="$5"
    local app_support session_root session_socket executable helper cli socket pane_id app_pid status log daemon_pid shell_pid
    pane_id="22222222-3333-4444-8555-666666666666"
    prepare_case "$case_root"
    app_support="$case_root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    write_persistent_fixture "$app_support"
    mark_fixture_established "$app_support"
    session_root="$(prepare_session_root)"
    session_socket="$session_root/control.sock"
    if [[ "$state_name" == unreachable ]]; then
        printf '%s\n' blocked > "$session_socket"
        chmod 0600 "$session_socket"
    fi
    executable="$staged_app/Contents/MacOS/MuxyTests"
    helper="$staged_app/Contents/MacOS/muxy-session-v2"
    cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    socket="$app_support/muxy-dev.sock"
    [[ "$mode" == release ]] && socket="$app_support/muxy.sock"
    log="$case_root/app.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P8_SESSION_SOCKET_PATH="$session_socket" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        MUXY_TEST_P8_RECOVERY_ACTION=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid=$!
    track_pid "$app_pid"
    trap cleanup_tracked EXIT
    wait_for_cli "$cli" "$socket" "$case_root/panes-before.txt" || {
        sed -n '1,240p' "$log"
        fail "$state_name recovery app did not become ready"
    }
    rg -q "^$pane_id" "$case_root/panes-before.txt" || fail "$state_name recovery changed the tab identity"
    if MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=3 \
        "$cli" read-screen --pane "$pane_id" --lines 20 > "$case_root/screen-before.txt" 2>&1; then
        fail "$state_name recovery unexpectedly materialized a terminal"
    fi
    if [[ "$state_name" == missing ]]; then
        printf '%s\n' "$pane_id" > "$app_support/.muxy-test-p8-start-fresh"
        wait_for_path "$session_socket" socket 200 || fail "Start Fresh did not create a session"
        wait_for_screen_text "$cli" "$socket" "$pane_id" '.' "$case_root/screen-after.txt" || \
            fail "Start Fresh did not materialize the retained tab"
        jq -e --arg pane "$pane_id" \
            '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
            "$app_support/workspaces.json" >/dev/null || fail "Start Fresh did not publish establishment"
        daemon_pid="$(ps -axo pid=,command= | awk -v socket="$session_socket" \
            '!found && $0 ~ /muxy-session-v2 daemon/ && $0 ~ /--socket/ && index($0, socket) { print $1; found = 1 }')"
        [[ "$daemon_pid" =~ ^[1-9][0-9]*$ ]] || fail "Start Fresh daemon PID was not found"
        track_session_daemon "$daemon_pid" "$helper" "$session_socket"
        shell_pid="$(session_shell_for_directory "$daemon_pid" "$app_support/project" || true)"
        [[ "$shell_pid" =~ ^[1-9][0-9]*$ ]] || fail "Start Fresh shell PID was not found"
        track_pid "$shell_pid" "$daemon_pid"
        close_pane_checked "$cli" "$socket" "$pane_id" >/dev/null
    else
        printf '%s\n' "$pane_id" > "$app_support/.muxy-test-p8-reconnect"
        wait_for_path "$app_support/.muxy-test-p8-reconnect" missing 100 || \
            fail "Reconnect action was not handled"
        local unavailable=false
        for _ in {1..200}; do
            if ! MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=3 \
                "$cli" read-screen --pane "$pane_id" --lines 20 > "$case_root/screen-after.txt" 2>&1; then
                unavailable=true
                break
            fi
            sleep 0.05
        done
        if [[ "$unavailable" != true ]]; then
            fail "unreachable Reconnect silently started a fresh shell"
        fi
        jq -e --arg pane "$pane_id" \
            '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
            "$app_support/workspaces.json" >/dev/null || fail "unreachable Reconnect changed durable session ownership"
    fi
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in {1..600}; do
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$app_pid" 2>/dev/null && fail "$state_name recovery app did not close normally"
    status=0
    wait "$app_pid" || status=$?
    [[ "$status" == 0 ]] || fail "$state_name recovery app exited with status $status"
    cleanup_tracked
    trap - EXIT
    remove_session_root "$session_root"
    snapshot_production "$case_root/production-after"
    compare_snapshots "$before" "$case_root/production-after"
}

run_recovery_states() {
    local mode="$1" source_app staged_app missing_root unreachable_root before
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-recovery-states")"
    validate_staged_app "$staged_app"
    missing_root="$VERIFICATION_ROOT/m"
    unreachable_root="$VERIFICATION_ROOT/u"
    before="$VERIFICATION_ROOT/rp"
    if [[ "$mode" == release ]]; then
        missing_root="$VERIFICATION_ROOT/rm"
        unreachable_root="$VERIFICATION_ROOT/ru"
        before="$VERIFICATION_ROOT/rrp"
    fi
    prepare_case "$before"
    snapshot_production "$before/snapshot"
    run_recovery_state_case "$mode" missing "$staged_app" "$missing_root" "$before/snapshot"
    run_recovery_state_case "$mode" unreachable "$staged_app" "$unreachable_root" "$before/snapshot"
    printf 'P8 staged %s missing and unreachable recovery passed\n' "$mode"
}

run_fallback_recovery() {
    local mode="$1" source_app staged_app case_root app_support session_root session_socket
    local executable helper cli socket pane_id attach_pid app_pid daemon_pid shell_pid status before after log
    pane_id="99999999-AAAA-4BBB-8CCC-DDDDDDDDDDDD"
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-fallback-recovery")"
    validate_staged_app "$staged_app"
    case_root="$VERIFICATION_ROOT/fr"
    [[ "$mode" == release ]] && case_root="$VERIFICATION_ROOT/rfr"
    prepare_case "$case_root"
    app_support="$case_root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    write_persistent_fixture "$app_support"
    printf '%s\n' '[]' > "$app_support/workspaces.json"
    chmod 0600 "$app_support/workspaces.json"
    session_root="$(prepare_session_root)"
    session_socket="$session_root/control.sock"
    executable="$staged_app/Contents/MacOS/MuxyTests"
    helper="$staged_app/Contents/MacOS/muxy-session-v2"
    cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    socket="$app_support/muxy-dev.sock"
    [[ "$mode" == release ]] && socket="$app_support/muxy.sock"
    before="$case_root/production-before"
    after="$case_root/production-after"
    snapshot_production "$before"
    MUXY_SESSION_SOCKET="$session_socket" \
        MUXY_SESSION_ID="$pane_id" \
        MUXY_SESSION_CREATE_POLICY=create-or-attach \
        MUXY_SESSION_PROJECT_ID="AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE" \
        MUXY_SESSION_WORKTREE_ID="11111111-2222-4333-8444-555555555555" \
        MUXY_SESSION_TITLE="Recovered Shell" \
        MUXY_SESSION_SHELL=/bin/sh \
        MUXY_SESSION_RESOURCES="$staged_app/Contents/Resources" \
        MUXY_SESSION_DIRECTORY="$app_support/project" \
    "$helper" attach </dev/null > "$case_root/attach.log" 2>&1 &
    attach_pid=$!
    track_pid "$attach_pid"
    trap cleanup_tracked EXIT
    wait_for_path "$session_socket" socket 200 || fail "ownerless fixture session was not created"
    daemon_pid="$(ps -axo pid=,command= | awk -v socket="$session_socket" \
        '!found && $0 ~ /muxy-session-v2 daemon/ && $0 ~ /--socket/ && index($0, socket) { print $1; found = 1 }')"
    [[ "$daemon_pid" =~ ^[1-9][0-9]*$ ]] || fail "ownerless fixture daemon PID was not found"
    track_session_daemon "$daemon_pid" "$helper" "$session_socket"
    shell_pid="$(session_shell_for_directory "$daemon_pid" "$app_support/project" || true)"
    [[ "$shell_pid" =~ ^[1-9][0-9]*$ ]] || fail "ownerless fixture shell PID was not found"
    track_pid "$shell_pid" "$daemon_pid"
    log="$case_root/app.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P8_SESSION_SOCKET_PATH="$session_socket" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid=$!
    track_pid "$app_pid"
    wait_for_cli "$cli" "$socket" "$case_root/panes.txt" || {
        sed -n '1,240p' "$log"
        fail "fallback recovery app did not become ready"
    }
    rg -q "^$pane_id" "$case_root/panes.txt" || fail "ownerless recovery did not create the fallback tab"
    wait_for_screen_text "$cli" "$socket" "$pane_id" '.' "$case_root/screen.txt" || \
        fail "ownerless fallback tab did not attach"
    jq -e --arg pane "$pane_id" \
        '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
        "$app_support/workspaces.json" >/dev/null || fail "fallback owner was not durably published"
    close_pane_checked "$cli" "$socket" "$pane_id" >/dev/null
    for _ in {1..200}; do
        ! kill -0 "$shell_pid" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$shell_pid" 2>/dev/null && fail "fallback tab close did not terminate its shell"
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in {1..600}; do
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$app_pid" 2>/dev/null && fail "fallback recovery app did not close normally"
    status=0
    wait "$app_pid" || status=$?
    [[ "$status" == 0 ]] || fail "fallback recovery app exited with status $status"
    cleanup_tracked
    trap - EXIT
    remove_session_root "$session_root"
    snapshot_production "$after"
    compare_snapshots "$before" "$after"
    printf 'P8 staged %s fallback recovery passed\n' "$mode"
}

run_enable_restart() {
    local mode="$1" source_app staged_app case_root app_support socket session_root session_socket
    local executable helper cli pane_id app_pid new_pid daemon_pid status state before after log directory
    pane_id="22222222-3333-4444-8555-666666666666"
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-enable-restart")"
    validate_staged_app "$staged_app"
    case_root="$VERIFICATION_ROOT/er"
    [[ "$mode" == release ]] && case_root="$VERIFICATION_ROOT/rer"
    prepare_case "$case_root"
    app_support="$case_root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    write_persistent_fixture "$app_support"
    jq '."muxy.terminalPersistentSession.enabled" = false' "$app_support/preferences.json" \
        > "$app_support/preferences-disabled.json"
    chmod 0600 "$app_support/preferences-disabled.json"
    mv "$app_support/preferences-disabled.json" "$app_support/preferences.json"
    session_root="$(prepare_session_root)"
    session_socket="$session_root/control.sock"
    executable="$staged_app/Contents/MacOS/MuxyTests"
    helper="$staged_app/Contents/MacOS/muxy-session-v2"
    cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    socket="$app_support/muxy-dev.sock"
    [[ "$mode" == release ]] && socket="$app_support/muxy.sock"
    before="$case_root/production-before"
    after="$case_root/production-after"
    snapshot_production "$before"
    log="$case_root/first.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P8_SESSION_SOCKET_PATH="$session_socket" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        MUXY_TEST_P8_ENABLE_RESTART=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid=$!
    track_pid "$app_pid"
    trap cleanup_tracked EXIT
    wait_for_cli "$cli" "$socket" "$case_root/before-panes.txt" || fail "enable fixture app did not become ready"
    wait_for_screen_text "$cli" "$socket" "$pane_id" '.' "$case_root/direct-screen.txt" || \
        fail "enable fixture did not begin with a direct terminal"
    [[ ! -e "$session_socket" ]] || fail "disabled fixture unexpectedly created a session socket"
    jq -S '.[0] | walk(if type == "object" then del(.paneTitle) else . end)' \
        "$app_support/workspaces.json" > "$case_root/layout-before.json"
    directory="$(jq -r --arg pane "$pane_id" \
        '.. | objects | select(.id? == $pane) | .terminalResumeDirectory // empty' \
        "$app_support/workspaces.json")"
    [[ "$directory" == "$app_support/project" ]] || fail "direct terminal directory was not captured before enable"
    printf '%s\n' enable > "$app_support/.muxy-test-p8-enable-restart"
    for _ in {1..400}; do
        state="$({ ps -o state= -p "$app_pid" 2>/dev/null || true; } | tr -d '[:space:]')"
        [[ -z "$state" || "$state" == Z* ]] && break
        sleep 0.05
    done
    state="$({ ps -o state= -p "$app_pid" 2>/dev/null || true; } | tr -d '[:space:]')"
    [[ -z "$state" || "$state" == Z* ]] || fail "enable transaction did not quit the old app"
    status=0
    wait "$app_pid" || status=$?
    [[ "$status" == 0 ]] || fail "enable transaction old app exited with status $status"
    for _ in {1..400}; do
        new_pid="$(ps -axo pid=,command= | awk -v executable="$executable" -v old="$app_pid" \
            '!found && $1 != old { pid = $1; $1 = ""; sub(/^ +/, ""); if ($0 == executable) { print pid; found = 1 } }')"
        [[ "$new_pid" =~ ^[1-9][0-9]*$ ]] && break
        sleep 0.05
    done
    [[ "$new_pid" =~ ^[1-9][0-9]*$ ]] || fail "enable restart helper did not launch a new staged app"
    track_relaunched_app "$new_pid" "$executable"
    wait_for_cli "$cli" "$socket" "$case_root/after-panes.txt" || fail "enabled app did not become ready"
    wait_for_path "$session_socket" socket 200 || fail "enabled app did not create the session service"
    for _ in {1..200}; do
        jq -e --arg pane "$pane_id" \
            '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
            "$app_support/workspaces.json" >/dev/null 2>&1 && break
        sleep 0.05
    done
    jq -e '."muxy.terminalPersistentSession.enabled" == true' \
        "$app_support/preferences.json" >/dev/null || fail "enable setting was not persisted"
    jq -e --arg pane "$pane_id" \
        '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
        "$app_support/workspaces.json" >/dev/null || fail "enabled session was not established"
    jq -e --arg pane "$pane_id" --arg directory "$directory" \
        '.. | objects | select(.id? == $pane) | .terminalResumeDirectory == $directory' \
        "$app_support/workspaces.json" >/dev/null || fail "enable restart changed the terminal directory"
    rg -q "^$pane_id" "$case_root/after-panes.txt" || fail "enable restart changed the tab identity"
    jq -S '.[0] | walk(if type == "object" then del(.rustPersistentSession, .paneTitle, .terminalResumeDirectory) else . end)' \
        "$app_support/workspaces.json" > "$case_root/layout-after.json"
    jq -S 'walk(if type == "object" then del(.terminalResumeDirectory) else . end)' \
        "$case_root/layout-before.json" > "$case_root/layout-before-normalized.json"
    cmp -s "$case_root/layout-before-normalized.json" "$case_root/layout-after.json" || \
        fail "enable restart changed the terminal layout"
    daemon_pid="$(ps -axo pid=,command= | awk -v socket="$session_socket" \
        '!found && $0 ~ /muxy-session-v2 daemon/ && $0 ~ /--socket/ && index($0, socket) { print $1; found = 1 }')"
    [[ "$daemon_pid" =~ ^[1-9][0-9]*$ ]] || fail "enabled session daemon PID was not found"
    track_session_daemon "$daemon_pid" "$helper" "$session_socket"
    track_descendants "$daemon_pid"
    close_pane_checked "$cli" "$socket" "$pane_id" >/dev/null
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in {1..600}; do
        ! kill -0 "$new_pid" 2>/dev/null && break
        sleep 0.05
    done
    state="$({ ps -o state= -p "$new_pid" 2>/dev/null || true; } | tr -d '[:space:]')"
    [[ -z "$state" || "$state" == Z* ]] || fail "enabled app did not close normally"
    cleanup_tracked
    trap - EXIT
    remove_session_root "$session_root"
    snapshot_production "$after"
    compare_snapshots "$before" "$after"
    printf 'P8 staged %s enable restart passed\n' "$mode"
}

run_disable_restart() {
    local mode="$1" source_app staged_app case_root app_support socket session_root session_socket
    local executable cli pane_id app_pid new_pid daemon_pid shell_pid status before after log state
    pane_id="22222222-3333-4444-8555-666666666666"
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-disable-restart")"
    validate_staged_app "$staged_app"
    case_root="$VERIFICATION_ROOT/dr"
    [[ "$mode" == release ]] && case_root="$VERIFICATION_ROOT/drr"
    prepare_case "$case_root"
    app_support="$case_root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    write_persistent_fixture "$app_support"
    session_root="$(prepare_session_root)"
    session_socket="$session_root/control.sock"
    executable="$staged_app/Contents/MacOS/MuxyTests"
    cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    socket="$app_support/muxy-dev.sock"
    [[ "$mode" == release ]] && socket="$app_support/muxy.sock"
    before="$case_root/production-before"
    after="$case_root/production-after"
    snapshot_production "$before"
    log="$case_root/first.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P8_SESSION_SOCKET_PATH="$session_socket" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        MUXY_TEST_P8_DISABLE_RESTART=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid=$!
    track_pid "$app_pid"
    trap cleanup_tracked EXIT
    wait_for_cli "$cli" "$socket" "$case_root/before-panes.txt" || fail "disable fixture app did not become ready"
    MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=3 \
        "$cli" read-screen --pane "$pane_id" --lines 20 > "$case_root/initial-screen.txt" || true
    wait_for_path "$session_socket" socket 200 || fail "disable fixture session socket was not created"
    for _ in {1..200}; do
        jq -e --arg pane "$pane_id" \
            '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
            "$app_support/workspaces.json" >/dev/null 2>&1 && break
        sleep 0.05
    done
    jq -e --arg pane "$pane_id" \
        '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
        "$app_support/workspaces.json" >/dev/null || fail "disable fixture session was not established"
    jq -S '.[0] | walk(if type == "object" then del(.rustPersistentSession, .paneTitle) else . end)' \
        "$app_support/workspaces.json" > "$case_root/layout-before.json"
    daemon_pid="$(ps -axo pid=,command= | awk -v socket="$session_socket" \
        '!found && $0 ~ /muxy-session-v2 daemon/ && $0 ~ /--socket/ && index($0, socket) { print $1; found = 1 }')"
    [[ "$daemon_pid" =~ ^[1-9][0-9]*$ ]] || fail "disable fixture daemon PID was not found"
    track_session_daemon "$daemon_pid" "$staged_app/Contents/MacOS/muxy-session-v2" "$session_socket"
    shell_pid="$(session_shell_for_directory "$daemon_pid" "$app_support/project" || true)"
    [[ "$shell_pid" =~ ^[1-9][0-9]*$ ]] || fail "disable fixture shell PID was not found"
    track_pid "$shell_pid" "$daemon_pid"
    printf '%s\n' disable > "$app_support/.muxy-test-p8-disable-restart"
    for _ in {1..400}; do
        state="$({ ps -o state= -p "$app_pid" 2>/dev/null || true; } | tr -d '[:space:]')"
        [[ -z "$state" || "$state" == Z* ]] && break
        sleep 0.05
    done
    state="$({ ps -o state= -p "$app_pid" 2>/dev/null || true; } | tr -d '[:space:]')"
    [[ -z "$state" || "$state" == Z* ]] || fail "disable transaction did not quit the old app"
    status=0
    wait "$app_pid" || status=$?
    [[ "$status" == 0 ]] || fail "disable transaction old app exited with status $status"
    for _ in {1..400}; do
        new_pid="$(ps -axo pid=,command= | awk -v executable="$executable" -v old="$app_pid" \
            '!found && $1 != old { pid = $1; $1 = ""; sub(/^ +/, ""); if ($0 == executable) { print pid; found = 1 } }')"
        [[ "$new_pid" =~ ^[1-9][0-9]*$ ]] && break
        sleep 0.05
    done
    [[ "$new_pid" =~ ^[1-9][0-9]*$ ]] || fail "restart helper did not launch a new staged app"
    track_relaunched_app "$new_pid" "$executable"
    wait_for_cli "$cli" "$socket" "$case_root/after-panes.txt" || fail "restarted direct-terminal app did not become ready"
    for _ in {1..200}; do
        jq -e '."muxy.terminalPersistentSession.enabled" == false' \
            "$app_support/preferences.json" >/dev/null 2>&1 && break
        sleep 0.05
    done
    jq -e '."muxy.terminalPersistentSession.enabled" == false' \
        "$app_support/preferences.json" >/dev/null || fail "disable setting was not persisted"
    if jq -e '.. | objects | .rustPersistentSession? == true' \
        "$app_support/workspaces.json" >/dev/null; then
        fail "disable restart retained a persistent session flag"
    fi
    jq -S '.[0] | walk(if type == "object" then del(.rustPersistentSession, .paneTitle) else . end)' \
        "$app_support/workspaces.json" > "$case_root/layout-after.json"
    cmp -s "$case_root/layout-before.json" "$case_root/layout-after.json" || \
        fail "disable restart changed the terminal layout"
    kill -0 "$shell_pid" 2>/dev/null && fail "disable acknowledgement left the daemon shell alive"
    rg -q "^$pane_id" "$case_root/after-panes.txt" || fail "disable restart changed the tab identity"
    wait_for_screen_text "$cli" "$socket" "$pane_id" '.' "$case_root/direct-screen.txt" || \
        fail "disable restart did not materialize a fresh direct terminal"
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in {1..600}; do
        ! kill -0 "$new_pid" 2>/dev/null && break
        sleep 0.05
    done
    state="$({ ps -o state= -p "$new_pid" 2>/dev/null || true; } | tr -d '[:space:]')"
    [[ -z "$state" || "$state" == Z* ]] || fail "restarted app did not close normally"
    cleanup_tracked
    trap - EXIT
    remove_session_root "$session_root"
    snapshot_production "$after"
    compare_snapshots "$before" "$after"
    printf 'P8 staged %s disable restart passed\n' "$mode"
}

run_restart_failure_case() {
    local mode="$1" point="$2" initially_enabled="$3" staged_app="$4" case_root="$5" before="$6"
    local app_support session_root session_socket executable helper cli socket pane_id app_pid daemon_pid shell_pid status log expected
    pane_id="22222222-3333-4444-8555-666666666666"
    prepare_case "$case_root"
    app_support="$case_root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    write_persistent_fixture "$app_support"
    if [[ "$initially_enabled" == false ]]; then
        jq '."muxy.terminalPersistentSession.enabled" = false' "$app_support/preferences.json" \
            > "$app_support/preferences-disabled.json"
        chmod 0600 "$app_support/preferences-disabled.json"
        mv "$app_support/preferences-disabled.json" "$app_support/preferences.json"
    fi
    session_root="$(prepare_session_root)"
    session_socket="$session_root/control.sock"
    executable="$staged_app/Contents/MacOS/MuxyTests"
    helper="$staged_app/Contents/MacOS/muxy-session-v2"
    cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    socket="$app_support/muxy-dev.sock"
    [[ "$mode" == release ]] && socket="$app_support/muxy.sock"
    log="$case_root/app.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P8_SESSION_SOCKET_PATH="$session_socket" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        MUXY_TEST_P8_ENABLE_RESTART=1 \
        MUXY_TEST_P8_DISABLE_RESTART=1 \
        MUXY_TEST_P8_RESTART_FAILURE="$point" \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
    "$executable" > "$log" 2>&1 &
    app_pid=$!
    track_pid "$app_pid"
    trap cleanup_tracked EXIT
    wait_for_cli "$cli" "$socket" "$case_root/panes.txt" || fail "$point app did not become ready"
    wait_for_screen_text "$cli" "$socket" "$pane_id" '.' "$case_root/screen.txt" || \
        fail "$point fixture terminal did not materialize"
    jq -S 'walk(if type == "object" then del(.paneTitle) else . end)' \
        "$app_support/workspaces.json" > "$case_root/layout-before.json"
    if [[ "$initially_enabled" == true ]]; then
        wait_for_path "$session_socket" socket 200 || fail "$point persistent fixture did not create a daemon"
        for _ in {1..200}; do
            jq -e --arg pane "$pane_id" \
                '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
                "$app_support/workspaces.json" >/dev/null 2>&1 && break
            sleep 0.05
        done
        daemon_pid="$(ps -axo pid=,command= | awk -v socket="$session_socket" \
            '!found && $0 ~ /muxy-session-v2 daemon/ && $0 ~ /--socket/ && index($0, socket) { print $1; found = 1 }')"
        [[ "$daemon_pid" =~ ^[1-9][0-9]*$ ]] || fail "$point daemon PID was not found"
        track_session_daemon "$daemon_pid" "$helper" "$session_socket"
        shell_pid="$(session_shell_for_directory "$daemon_pid" "$app_support/project" || true)"
        [[ "$shell_pid" =~ ^[1-9][0-9]*$ ]] || fail "$point shell PID was not found"
        track_pid "$shell_pid" "$daemon_pid"
        printf '%s\n' disable > "$app_support/.muxy-test-p8-disable-restart"
        wait_for_path "$app_support/.muxy-test-p8-disable-restart" missing 100 || \
            fail "$point disable request was not handled"
        for _ in {1..240}; do
            ! kill -0 "$shell_pid" 2>/dev/null && break
            sleep 0.05
        done
        kill -0 "$shell_pid" 2>/dev/null && fail "$point did not reach the post-termination boundary"
        expected=true
    else
        printf '%s\n' enable > "$app_support/.muxy-test-p8-enable-restart"
        wait_for_path "$app_support/.muxy-test-p8-enable-restart" missing 100 || \
            fail "$point enable request was not handled"
        sleep 2.5
        expected=false
    fi
    kill -0 "$app_pid" 2>/dev/null || fail "$point restarted the app after an injected failure"
    for _ in {1..200}; do
        jq -e --argjson expected "$expected" \
            '."muxy.terminalPersistentSession.enabled" == $expected' \
            "$app_support/preferences.json" >/dev/null 2>&1 && break
        sleep 0.05
    done
    jq -e --argjson expected "$expected" \
        '."muxy.terminalPersistentSession.enabled" == $expected' \
        "$app_support/preferences.json" >/dev/null || fail "$point did not restore the persistent setting"
    if [[ "$initially_enabled" == true ]]; then
        jq -e --arg pane "$pane_id" \
            '.. | objects | select(.id? == $pane) | .rustPersistentSession == true' \
            "$app_support/workspaces.json" >/dev/null || fail "$point did not restore durable tab ownership"
    elif jq -e '.. | objects | .rustPersistentSession? == true' \
        "$app_support/workspaces.json" >/dev/null; then
        fail "$point published persistent tab ownership"
    fi
    jq -S 'walk(if type == "object" then del(.paneTitle) else . end)' \
        "$app_support/workspaces.json" > "$case_root/layout-after.json"
    cmp -s "$case_root/layout-before.json" "$case_root/layout-after.json" || \
        fail "$point did not restore the durable workspace snapshot"
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in {1..600}; do
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$app_pid" 2>/dev/null && fail "$point app did not close normally"
    status=0
    wait "$app_pid" || status=$?
    [[ "$status" == 0 ]] || fail "$point app exited with status $status"
    cleanup_tracked
    trap - EXIT
    remove_session_root "$session_root"
    snapshot_production "$case_root/production-after"
    compare_snapshots "$before" "$case_root/production-after"
}

run_restart_failures() {
    local mode="$1" source_app staged_app before code index point enabled
    source_app="$PROJECT_ROOT/target/$mode/Muxy.app"
    [[ -d "$source_app" ]] || fail "bundle not found: $source_app"
    staged_app="$("$SCRIPT_DIR/stage-test-app.sh" "$source_app" "p8-$mode-restart-failures")"
    validate_staged_app "$staged_app"
    before="$VERIFICATION_ROOT/fp"
    [[ "$mode" == release ]] && before="$VERIFICATION_ROOT/rfp"
    prepare_case "$before"
    snapshot_production "$before/snapshot"
    index=0
    for point in enable-after-setting enable-commit disable-after-workspace disable-after-setting disable-commit; do
        enabled=false
        [[ "$point" == disable-* ]] && enabled=true
        code="$VERIFICATION_ROOT/f$index"
        [[ "$mode" == release ]] && code="$VERIFICATION_ROOT/rf$index"
        run_restart_failure_case "$mode" "$point" "$enabled" "$staged_app" "$code" "$before/snapshot"
        ((index += 1))
    done
    printf 'P8 staged %s restart failure boundaries passed\n' "$mode"
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

for command_name in awk cmp cut find head jq lsof mktemp pgrep ps readlink rg sed shasum sort stat tr wc; do
    require_command "$command_name"
done

case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p8-terminal-memory.sh --self-test"
        self_test
        ;;
    --fixture)
        (($# == 2)) || fail "usage: scripts/verify-p8-terminal-memory.sh --fixture CASE"
        [[ "$2" == scope || "$2" == persistent-contracts ]] || fail "unsupported fixture: $2"
        if [[ "$2" == scope ]]; then
            scope_checks
        else
            persistent_contracts
        fi
        ;;
    --mode)
        (($# == 4)) || fail "usage: scripts/verify-p8-terminal-memory.sh --mode PROFILE --case CASE"
        [[ "$2" == debug || "$2" == release ]] || fail "profile must be debug or release"
        [[ "$3" == --case ]] || fail "missing staged case"
        [[ "$4" == launch-close || "$4" == daemon-harness || "$4" == crash-recovery || "$4" == normal-recovery || "$4" == recovery-states || "$4" == fallback-recovery || "$4" == enable-restart || "$4" == disable-restart || "$4" == restart-failures ]] || fail "unsupported staged case"
        for command_name in cargo codesign plutil; do
            require_command "$command_name"
        done
        case "$4" in
            launch-close) run_launch_close "$2" ;;
            daemon-harness) run_daemon_harness "$2" ;;
            crash-recovery) run_crash_recovery "$2" ;;
            normal-recovery) run_normal_recovery "$2" ;;
            recovery-states) run_recovery_states "$2" ;;
            fallback-recovery) run_fallback_recovery "$2" ;;
            enable-restart) run_enable_restart "$2" ;;
            disable-restart) run_disable_restart "$2" ;;
            restart-failures) run_restart_failures "$2" ;;
        esac
        ;;
    *)
        fail "usage: scripts/verify-p8-terminal-memory.sh --self-test | --fixture scope|persistent-contracts | --mode PROFILE --case CASE"
        ;;
esac
