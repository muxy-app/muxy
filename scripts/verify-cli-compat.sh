#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly SHIM="${1:-$(command -v muxy || true)}"
readonly ROOT="$PROJECT_ROOT/target/test-verification/p3-cli"
readonly APP_SUPPORT="$ROOT/app-support"
readonly XDG_CONFIG_ROOT="$ROOT/xdg-config"
readonly PRODUCTION_SOCKET="${MUXY_PRODUCTION_SOCKET_PATH:-$HOME/Library/Application Support/Muxy/muxy.sock}"
readonly SOURCE_CLI="$PROJECT_ROOT/Muxy/Resources/scripts/muxy-cli"
readonly DEBUG_BUNDLE="$PROJECT_ROOT/target/debug/Muxy.app"
readonly RELEASE_BUNDLE="$PROJECT_ROOT/target/release/Muxy.app"
readonly APP="$PROJECT_ROOT/target/test-verification/apps/p3-cli-debug/MuxyTests.app"
readonly RELEASE_APP="$PROJECT_ROOT/target/test-verification/apps/p3-cli-release/MuxyTests.app"
readonly APP_EXECUTABLE="$APP/Contents/MacOS/MuxyTests"
readonly DEVELOPMENT_SOCKET="$APP_SUPPORT/muxy-dev.sock"
readonly COMMAND_LOG="$ROOT/commands.log"
readonly ACCEPTED_LOG="$ROOT/accepted-heads.log"
readonly PROJECT_SETUP_SENTINEL="$ROOT/project-setup-ran"
readonly GLOBAL_SETUP_SENTINEL="$ROOT/global-setup-ran"
APP_PID=""

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

cleanup() {
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
        kill -TERM "$APP_PID" 2>/dev/null || true
        wait "$APP_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

for command_name in base64 cmp codesign cut diff git grep head jot lsof nc open osascript plutil shasum sort stat; do
    command -v "$command_name" >/dev/null 2>&1 || fail "required command not found: $command_name"
done
[[ -x "$SHIM" ]] || fail "installed shim is missing or not executable: $SHIM"
[[ "$SHIM" != "$SOURCE_CLI" ]] || fail "acceptance must enter through the installed shim"
grep -Fq 'Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli' "$SHIM" || {
    fail "installed shim does not contain the literal legacy path"
}
grep -Fq 'MUXY_APP_PATH' "$SHIM" || fail "installed shim does not honor MUXY_APP_PATH"
[[ -S "$PRODUCTION_SOCKET" ]] || fail "production Swift socket is not live"
PRODUCTION_IDENTITY="$(stat -f '%d:%i' "$PRODUCTION_SOCKET")"
readonly PRODUCTION_IDENTITY
PRODUCTION_PID="$(lsof -t "$PRODUCTION_SOCKET" | head -n 1)"
readonly PRODUCTION_PID
[[ -n "$PRODUCTION_PID" ]] || fail "production socket has no owning process"
kill -0 "$PRODUCTION_PID" 2>/dev/null || fail "production socket owner is not live"

"$SCRIPT_DIR/build-app.sh" debug
"$SCRIPT_DIR/verify-bundle.sh" "$DEBUG_BUNDLE" debug
"$SCRIPT_DIR/stage-test-app.sh" "$DEBUG_BUNDLE" p3-cli-debug >/dev/null
"$SCRIPT_DIR/build-app.sh" release
"$SCRIPT_DIR/verify-bundle.sh" "$RELEASE_BUNDLE" release
"$SCRIPT_DIR/stage-test-app.sh" "$RELEASE_BUNDLE" p3-cli-release >/dev/null
readonly DEBUG_CLI="$DEBUG_BUNDLE/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
readonly RELEASE_CLI="$RELEASE_BUNDLE/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
readonly STAGED_CLI="$APP/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
readonly STAGED_RELEASE_CLI="$RELEASE_APP/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
for bundled_cli in "$DEBUG_CLI" "$RELEASE_CLI" "$STAGED_CLI" "$STAGED_RELEASE_CLI"; do
    [[ -x "$bundled_cli" ]] || fail "bundled CLI is not executable: $bundled_cli"
    cmp -s "$SOURCE_CLI" "$bundled_cli" || fail "bundled CLI differs from retained source: $bundled_cli"
done

rm -rf "$ROOT/app-support" "$ROOT/fixtures" "$ROOT/logs" "$XDG_CONFIG_ROOT"
rm -f "$PROJECT_SETUP_SENTINEL" "$GLOBAL_SETUP_SENTINEL"
mkdir -p "$APP_SUPPORT" "$ROOT/fixtures" "$ROOT/logs" "$XDG_CONFIG_ROOT/muxy"
: > "$COMMAND_LOG"
: > "$ACCEPTED_LOG"

create_repository() {
    local name="$1"
    local repository="$ROOT/fixtures/$name"
    mkdir -p "$repository"
    git -C "$repository" init -q -b main
    git -C "$repository" config user.email muxy-tests@example.invalid
    git -C "$repository" config user.name MuxyTests
    printf '%s\n' "$name" > "$repository/README.md"
    git -C "$repository" add README.md
    git -C "$repository" commit -q -m initial
    if [[ "$name" != gamma && "$name" != delta ]]; then
        git -C "$repository" worktree add -q -b shared "$ROOT/fixtures/$name-shared"
    fi
}

create_repository alpha
create_repository beta
create_repository gamma
create_repository delta
mkdir -p "$ROOT/fixtures/gamma/.muxy"
printf '{"setup":[{"command":"touch %s","name":"Project setup"}]}\n' \
    "$PROJECT_SETUP_SENTINEL" > "$ROOT/fixtures/gamma/.muxy/worktree.json"
printf '{"setup":[{"command":"touch %s","name":"Per-machine setup"}]}\n' \
    "$GLOBAL_SETUP_SENTINEL" > "$XDG_CONFIG_ROOT/muxy/worktree.json"

MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$APP_SUPPORT" \
    XDG_CONFIG_HOME="$XDG_CONFIG_ROOT" \
    MUXY_PANE_ID="STALE-PANE" \
    MUXY_PROJECT_ID="STALE-PROJECT" \
    MUXY_WORKTREE_ID="STALE-WORKTREE" \
    MUXY_SOCKET_PATH="/tmp/stale-muxy.sock" \
    MUXY_HOOK_BIN="/tmp/stale-hook" \
    MUXY_HOOK_SCRIPT="/tmp/stale-hook.sh" \
    "$APP_EXECUTABLE" > "$ROOT/logs/app.log" 2>&1 &
APP_PID=$!
for _ in $(jot 400); do
    [[ -S "$DEVELOPMENT_SOCKET" ]] && break
    kill -0 "$APP_PID" 2>/dev/null || fail "staged app exited before binding"
    sleep 0.05
done
[[ -S "$DEVELOPMENT_SOCKET" ]] || fail "development socket was not created"
[[ "$(stat -f '%Lp' "$DEVELOPMENT_SOCKET")" == 600 ]] || fail "development socket mode is not 0600"
[[ "$(stat -f '%d:%i' "$PRODUCTION_SOCKET")" == "$PRODUCTION_IDENTITY" ]] || {
    fail "production socket identity changed during staged launch"
}
kill -0 "$PRODUCTION_PID" 2>/dev/null || fail "production Swift exited during staged launch"
MUXY_APP_PATH="$APP" MUXY_SOCKET_PATH="$DEVELOPMENT_SOCKET" MUXY_CLI_TIMEOUT=5 \
    bash -x "$SHIM" list-projects > "$ROOT/logs/shim-trace.out" 2>&1 || {
    fail "installed shim runtime resolution probe failed"
}
grep -Fq "exec $STAGED_CLI list-projects" "$ROOT/logs/shim-trace.out" || {
    fail "installed shim did not resolve the staged nested CLI"
}

run_cli() {
    {
        printf 'muxy'
        printf ' %q' "$@"
        printf '\n'
    } >> "$COMMAND_LOG"
    local output
    output="$(MUXY_APP_PATH="$APP" MUXY_SOCKET_PATH="$DEVELOPMENT_SOCKET" \
        MUXY_CLI_TIMEOUT=5 "$SHIM" "$@" 2>> "$COMMAND_LOG")" || return $?
    printf '%s\n' "$output" >> "$COMMAND_LOG"
    printf '%s' "$output"
}

accept() {
    local head="$1"
    shift
    local output
    output="$(run_cli "$@")" || fail "wrapper command failed for $head"
    printf '%s\n' "$head" >> "$ACCEPTED_LOG"
    printf '%s' "$output"
}

expect_ok() {
    local output="$1"
    [[ "$output" == ok || "$output" == ok$'\t'* ]] || fail "expected ok reply, got: $output"
}

team_reply="$(accept create-workspace create-workspace Team)"
expect_ok "$team_reply"
alpha_reply="$(accept create-project create-project "$ROOT/fixtures/alpha" --name Alpha)"
expect_ok "$alpha_reply"
beta_reply="$(accept create-project create-project "$ROOT/fixtures/beta" --name Beta --workspace Team)"
expect_ok "$beta_reply"
gamma_reply="$(accept create-project create-project "$ROOT/fixtures/gamma" --name Gamma --workspace Team)"
expect_ok "$gamma_reply"
alpha_id="$(printf '%s' "$alpha_reply" | cut -f2)"
beta_id="$(printf '%s' "$beta_reply" | cut -f2)"
gamma_id="$(printf '%s' "$gamma_reply" | cut -f2)"
[[ "$alpha_id" =~ ^[0-9A-F-]{36}$ && "$beta_id" =~ ^[0-9A-F-]{36}$ && "$gamma_id" =~ ^[0-9A-F-]{36}$ ]] || {
    fail "project IDs were not uppercase UUIDs"
}
projects="$(accept list-projects list-projects)"
printf '%s\n' "$projects" | grep -Fq $'\tAlpha\t' || fail "Alpha missing from list-projects"
printf '%s\n' "$projects" | grep -Fq $'\tBeta\t' || fail "Beta missing from list-projects"
created_path="$ROOT/fixtures/gamma-created"
created_reply="$(accept create-worktree create-worktree "Compat Create" \
    --branch compat/create --base main --project Gamma --path "$created_path")"
created_tabs=0
created_remainder="$created_reply"
while [[ "$created_remainder" == *$'\t'* ]]; do
    created_remainder="${created_remainder#*$'\t'}"
    created_tabs=$((created_tabs + 1))
done
[[ "$created_tabs" == 4 && "$created_reply" != *$'\n'* ]] || {
    fail "create-worktree did not return exactly five fields"
}
IFS=$'\t' read -r created_status created_id created_name created_reply_path \
    created_branch <<< "$created_reply"
[[ "$created_status" == ok ]] || fail "create-worktree status differed"
[[ "$created_id" =~ ^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$ ]] || {
    fail "created worktree ID is not an uppercase UUID"
}
[[ "$created_name" == "Compat Create" ]] || fail "created worktree name differed"
[[ "$created_reply_path" == "$created_path" ]] || fail "created worktree path differed"
[[ "$created_branch" == "compat/create" ]] || fail "created worktree branch differed"
[[ -d "$created_path" ]] || fail "explicit worktree path was not created"
created_worktrees="$(accept list-worktrees list-worktrees Gamma)"
printf '%s\n' "$created_worktrees" | \
    grep -Fxq "$created_id"$'\t'"Compat Create"$'\t'"$created_path"$'\t'"compat/create"$'\ttrue' || {
    fail "created worktree was not visible and active before the reply path continued"
}
[[ ! -e "$PROJECT_SETUP_SENTINEL" ]] || fail "CLI create-worktree ran project setup"
[[ ! -e "$GLOBAL_SETUP_SENTINEL" ]] || fail "CLI create-worktree ran per-machine setup"
expect_ok "$(accept switch-project switch-project Alpha)"
alpha_refresh="$(accept refresh-worktrees refresh-worktrees Alpha)"
[[ "$alpha_refresh" == ok$'\t'* ]] || fail "refresh-worktrees did not return a count"
beta_refresh="$(accept refresh-worktrees refresh-worktrees Beta)"
[[ "$beta_refresh" == ok$'\t'* ]] || fail "Beta worktree refresh failed"
worktrees="$(accept list-worktrees list-worktrees Alpha)"
printf '%s\n' "$worktrees" | grep -Fq $'\tshared\t' || fail "linked Alpha worktree missing"
alpha_shared_id="$(printf '%s\n' "$worktrees" | grep -F $'\tshared\t' | head -n 1 | cut -f1)"
[[ "$alpha_shared_id" =~ ^[0-9A-F-]{36}$ ]] || fail "linked Alpha worktree ID is invalid"
expect_ok "$(accept switch-worktree switch-worktree shared --project Alpha)"
workspaces="$(accept list-workspaces list-workspaces)"
printf '%s\n' "$workspaces" | grep -Fq $'\tTeam\t' || fail "Team missing from list-workspaces"
expect_ok "$(accept switch-workspace switch-workspace Team)"
expect_ok "$(accept rename-workspace rename-workspace Team Squad)"
temp_reply="$(accept create-workspace create-workspace Temp)"
expect_ok "$temp_reply"
expect_ok "$(accept switch-workspace switch-workspace Temp)"
expect_ok "$(accept switch-workspace switch-workspace Squad)"
expect_ok "$(accept delete-workspace delete-workspace Temp)"
expect_ok "$(accept attach-project attach-project Alpha --workspace Squad)"
expect_ok "$(accept detach-project detach-project Beta)"
expect_ok "$(accept attach-project attach-project Beta --workspace Squad)"
expect_ok "$(accept switch-project switch-project Gamma)"

if MUXY_APP_PATH="$APP" MUXY_SOCKET_PATH="$DEVELOPMENT_SOCKET" MUXY_CLI_TIMEOUT=5 \
    "$SHIM" list-tabs --worktree shared > "$ROOT/logs/ambiguous.out" 2>&1; then
    fail "ambiguous worktree targeting unexpectedly succeeded"
fi
grep -Fqi 'ambiguous across projects' "$ROOT/logs/ambiguous.out" || {
    fail "ambiguous worktree error was not retained"
}

active_tab="$(accept new-tab new-tab)"
[[ "$active_tab" =~ ^[0-9A-F-]{36}$ ]] || fail "new-tab did not return an uppercase UUID"
alpha_tab="$(accept new-tab new-tab --project Alpha)"
shared_tab="$(accept new-tab new-tab --project Alpha --worktree shared)"
beta_tab="$(accept new-tab new-tab --project Beta)"
for tab_id in "$alpha_tab" "$shared_tab" "$beta_tab"; do
    [[ "$tab_id" =~ ^[0-9A-F-]{36}$ ]] || fail "targeted new-tab did not return an uppercase UUID"
done
tabs="$(accept list-tabs list-tabs)"
printf '%s\n' "$tabs" | grep -Fq "$active_tab" || fail "active new tab missing"
expect_ok "$(accept switch-tab switch-tab "$active_tab")"
expect_ok "$(accept next-tab next-tab --project Alpha)"
expect_ok "$(accept previous-tab previous-tab --project Alpha --worktree shared)"

gamma_pane="$(accept split-right split-right)"
alpha_pane="$(accept split-down split-down --project Alpha)"
shared_pane="$(accept split-right split-right --project Alpha --worktree shared)"
for pane_id in "$gamma_pane" "$alpha_pane" "$shared_pane"; do
    [[ "$pane_id" =~ ^[0-9A-F-]{36}$ ]] || fail "split did not return an uppercase UUID"
done
printf -v environment_probe 'printf "MUXY_ENV_BEGIN\\nPANE=%%s\\nPROJECT=%%s\\nWORKTREE=%%s\\nSOCKET=%%s\\nHOOK_BIN=%%s\\nHOOK_SCRIPT=%%s\\nMUXY_ENV_END\\n" "%sMUXY_PANE_ID" "%sMUXY_PROJECT_ID" "%sMUXY_WORKTREE_ID" "%sMUXY_SOCKET_PATH" "%s{MUXY_HOOK_BIN-unset}" "%s{MUXY_HOOK_SCRIPT-unset}"' '$' '$' '$' '$' '$' '$'
readonly ENVIRONMENT_PROBE="$environment_probe"
expect_ok "$(accept send send --pane "$alpha_tab" "$ENVIRONMENT_PROBE")"
expect_ok "$(accept send-keys send-keys --pane "$alpha_tab" Enter)"
hidden_screen=""
for _ in $(jot 100); do
    hidden_screen="$(accept read-screen read-screen --pane "$alpha_tab" --lines 30)"
    [[ "$hidden_screen" == *"PANE=$alpha_tab"* ]] && break
    sleep 0.05
done
[[ "$hidden_screen" == *"PANE=$alpha_tab"* ]] || fail "hidden pane did not receive its pane ID"
[[ "$hidden_screen" == *"PROJECT=$alpha_id"* ]] || fail "hidden pane did not receive its project ID"
[[ "$hidden_screen" == *"WORKTREE=$alpha_shared_id"* ]] || fail "hidden pane did not receive its worktree ID"
[[ "$hidden_screen" == *"SOCKET=$DEVELOPMENT_SOCKET"* ]] || fail "hidden pane did not receive the selected socket"
[[ "$hidden_screen" == *"HOOK_BIN=unset"* ]] || fail "hidden pane inherited MUXY_HOOK_BIN"
[[ "$hidden_screen" == *"HOOK_SCRIPT=unset"* ]] || fail "hidden pane inherited MUXY_HOOK_SCRIPT"
readonly SOURCE_DIRECTORY="$ROOT/fixtures/source-cwd"
mkdir -p "$SOURCE_DIRECTORY"
printf -v cwd_command 'cd %q && printf "MUXY_CWD_READY\\n"' "$SOURCE_DIRECTORY"
expect_ok "$(accept send send --pane "$alpha_tab" "$cwd_command")"
expect_ok "$(accept send-keys send-keys --pane "$alpha_tab" Enter)"
for _ in $(jot 100); do
    hidden_screen="$(accept read-screen read-screen --pane "$alpha_tab" --lines 30)"
    printf '%s\n' "$hidden_screen" | grep -Fxq MUXY_CWD_READY && break
    sleep 0.05
done
printf '%s\n' "$hidden_screen" | grep -Fxq MUXY_CWD_READY || fail "hidden pane did not change working directory"
cwd_reported=false
for _ in $(jot 100); do
    if run_cli list-panes | grep -F "$alpha_tab" | grep -F "$SOURCE_DIRECTORY" | grep -Fq $'\tfalse'; then
        cwd_reported=true
        break
    fi
    sleep 0.05
done
[[ "$cwd_reported" == true ]] || fail "hidden inactive pane metadata did not report its working directory"
printf -v source_cwd_command 'printf "MUXY_SOURCE_CWD:%%s\\n" "%sPWD"' '$'
source_pane="$(accept split-right split-right --from "$alpha_tab" "$source_cwd_command")"
[[ "$source_pane" =~ ^[0-9A-F-]{36}$ ]] || fail "source split did not return an uppercase UUID"
source_screen=""
for _ in $(jot 100); do
    source_screen="$(accept read-screen read-screen --pane "$source_pane" --lines 20)"
    printf '%s\n' "$source_screen" | grep -Fxq "MUXY_SOURCE_CWD:$SOURCE_DIRECTORY" && break
    sleep 0.05
done
printf '%s\n' "$source_screen" | grep -Fxq "MUXY_SOURCE_CWD:$SOURCE_DIRECTORY" || fail "split did not inherit the explicit source pane working directory"
expect_ok "$(accept send send --pane "$gamma_pane" 'printf "MUXY_PHASE7\n"')"
expect_ok "$(accept send-keys send-keys --pane "$gamma_pane" Enter)"
screen=""
for _ in $(jot 100); do
    screen="$(accept read-screen read-screen --pane "$gamma_pane" --lines 20)"
    printf '%s\n' "$screen" | grep -Fxq MUXY_PHASE7 && break
    sleep 0.05
done
printf '%s\n' "$screen" | grep -Fxq MUXY_PHASE7 || fail "read-screen did not observe sent text"
expect_ok "$(accept rename-pane rename-pane --pane "$gamma_pane" CompatPane)"
panes="$(accept list-panes list-panes)"
printf '%s\n' "$panes" | grep -Fq "$gamma_pane" || fail "split pane missing from list-panes"
expect_ok "$(accept close-pane close-pane --pane "$alpha_pane")"
expect_ok "$(accept close-pane close-pane --pane "$shared_pane")"
expect_ok "$(accept close-pane close-pane --pane "$source_pane")"
expect_ok "$(accept close-pane close-pane --pane "$gamma_pane")"

expect_ok "$(accept tab-rename tab rename "$active_tab" CompatTab)"
expect_ok "$(accept tab-set-color tab set-color "$active_tab" blue)"
expect_ok "$(accept tab-set-icon tab set-icon "$active_tab" star.fill)"
expect_ok "$(accept tab-pin tab pin "$active_tab")"
expect_ok "$(accept tab-unpin tab unpin "$active_tab")"
expect_ok "$(accept tab-move tab move "$active_tab" 0)"
expect_ok "$(accept tab-close tab close "$active_tab")"

run_cli "$ROOT/fixtures/delta" >/dev/null || fail "path-open wrapper invocation failed"
path_opened=false
for _ in $(jot 100); do
    if run_cli list-projects | grep -F "$ROOT/fixtures/delta" | grep -Fq $'\ttrue'; then
        path_opened=true
        break
    fi
    sleep 0.05
done
[[ "$path_opened" == true ]] || fail "path-open did not add and select the fresh project"
printf '%s\n' path-open >> "$ACCEPTED_LOG"
readonly HOOK='{"v":3,"kind":"agent_event","id":"p2-cli-hook","provider":"compat","phase":"finished","title":"Done","body":"Ready","pids":[],"ts":7,"test":true}'
printf '%s\n' "$HOOK" | nc -w 2 -U "$DEVELOPMENT_SOCKET" > "$ROOT/logs/hook.out"
[[ "$(cat "$ROOT/logs/hook.out")" == '{"kind":"ack","ok":true,"v":3}' ]] || {
    fail "raw hook acknowledgement differed"
}
printf '%s\n' 'finished||Compat Notice|Body|with|pipes' | \
    nc -w 1 -U "$DEVELOPMENT_SOCKET" > "$ROOT/logs/notification.out"
[[ ! -s "$ROOT/logs/notification.out" ]] || fail "legacy notification returned a response"
printf '%s\n' raw-hook raw-notification >> "$ACCEPTED_LOG"

cat > "$ROOT/expected-heads.txt" <<'HEADS'
attach-project
close-pane
create-project
create-workspace
create-worktree
delete-workspace
detach-project
list-panes
list-projects
list-tabs
list-workspaces
list-worktrees
new-tab
next-tab
previous-tab
read-screen
refresh-worktrees
rename-pane
rename-workspace
send
send-keys
split-down
split-right
switch-project
switch-tab
switch-workspace
switch-worktree
tab-close
tab-move
tab-pin
tab-rename
tab-set-color
tab-set-icon
tab-unpin
HEADS
sort -u "$ACCEPTED_LOG" | grep -v -E '^(path-open|raw-hook|raw-notification)$' > "$ROOT/observed-heads.txt"
diff -u "$ROOT/expected-heads.txt" "$ROOT/observed-heads.txt" > "$ROOT/logs/head-diff.out" || {
    fail "not every P2 accepted wire head ran through the installed shim"
}

osascript -e 'tell application id "com.muxy.tests" to quit' >/dev/null
for _ in $(jot 400); do
    ! kill -0 "$APP_PID" 2>/dev/null && break
    sleep 0.05
done
kill -0 "$APP_PID" 2>/dev/null && fail "staged app did not quit normally"
set +e
wait "$APP_PID"
readonly APP_STATUS=$?
set -e
APP_PID=""
[[ "$APP_STATUS" == 0 ]] || fail "staged app exited with status $APP_STATUS"
[[ ! -e "$DEVELOPMENT_SOCKET" ]] || fail "development socket remained after shutdown"
[[ -S "$PRODUCTION_SOCKET" ]] || fail "production socket disappeared"
[[ "$(stat -f '%d:%i' "$PRODUCTION_SOCKET")" == "$PRODUCTION_IDENTITY" ]] || {
    fail "production socket identity changed"
}
kill -0 "$PRODUCTION_PID" 2>/dev/null || fail "production Swift is no longer live"
[[ "$(lsof -t "$PRODUCTION_SOCKET" | head -n 1)" == "$PRODUCTION_PID" ]] || {
    fail "production socket owner changed"
}

printf 'installed shim: %s\n' "$SHIM"
printf 'staged debug app: %s\n' "$APP"
printf 'staged release app: %s\n' "$RELEASE_APP"
printf 'development socket mode: 0600\n'
printf 'accepted wire heads: 34\n'
printf 'create-worktree exact reply and active selection: passed\n'
printf 'create-worktree setup hooks skipped: passed\n'
printf 'pane context and hidden materialization: passed\n'
printf 'explicit source working directory: passed\n'
printf 'path-open mutation: passed\n'
printf 'raw hook: passed\n'
printf 'raw notification: passed\n'
printf 'production socket preserved: %s pid %s\n' "$PRODUCTION_IDENTITY" "$PRODUCTION_PID"
printf 'normal staged shutdown: passed\n'
