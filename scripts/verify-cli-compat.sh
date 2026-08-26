#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly SHIM="${1:-$(command -v muxy || true)}"
readonly ROOT="$PROJECT_ROOT/target/test-verification/p2-cli"
readonly APP_SUPPORT="$ROOT/app-support"
readonly PRODUCTION_SOCKET="${MUXY_PRODUCTION_SOCKET_PATH:-$HOME/Library/Application Support/Muxy/muxy.sock}"
readonly SOURCE_CLI="$PROJECT_ROOT/Muxy/Resources/scripts/muxy-cli"
readonly APP="$PROJECT_ROOT/target/test-verification/apps/p2-cli/MuxyTests.app"
readonly APP_EXECUTABLE="$APP/Contents/MacOS/MuxyTests"
readonly DEVELOPMENT_SOCKET="$APP_SUPPORT/muxy-dev.sock"
readonly COMMAND_LOG="$ROOT/commands.log"
readonly ACCEPTED_LOG="$ROOT/accepted-heads.log"
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
"$SCRIPT_DIR/verify-bundle.sh" "$PROJECT_ROOT/target/debug/Muxy.app"
"$SCRIPT_DIR/stage-test-app.sh" "$PROJECT_ROOT/target/debug/Muxy.app" p2-cli >/dev/null
readonly STAGED_CLI="$APP/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
[[ -x "$STAGED_CLI" ]] || fail "staged nested CLI is not executable"
cmp -s "$SOURCE_CLI" "$STAGED_CLI" || fail "staged nested CLI differs from retained source"

rm -rf "$ROOT/app-support" "$ROOT/fixtures" "$ROOT/logs"
mkdir -p "$APP_SUPPORT" "$ROOT/fixtures" "$ROOT/logs"
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
    if [[ "$name" != gamma ]]; then
        git -C "$repository" worktree add -q -b shared "$ROOT/fixtures/$name-shared"
    fi
}

create_repository alpha
create_repository beta
create_repository gamma

MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$APP_SUPPORT" \
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
expect_ok "$(accept switch-project switch-project Alpha)"
alpha_refresh="$(accept refresh-worktrees refresh-worktrees Alpha)"
[[ "$alpha_refresh" == ok$'\t'* ]] || fail "refresh-worktrees did not return a count"
beta_refresh="$(accept refresh-worktrees refresh-worktrees Beta)"
[[ "$beta_refresh" == ok$'\t'* ]] || fail "Beta worktree refresh failed"
worktrees="$(accept list-worktrees list-worktrees Alpha)"
printf '%s\n' "$worktrees" | grep -Fq $'\tshared\t' || fail "linked Alpha worktree missing"
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
expect_ok "$(accept send send --pane "$gamma_pane" 'printf MUXY_PHASE7')"
expect_ok "$(accept send-keys send-keys --pane "$gamma_pane" Enter)"
screen=""
for _ in $(jot 100); do
    screen="$(accept read-screen read-screen --pane "$gamma_pane" --lines 20)"
    [[ "$screen" == *MUXY_PHASE7* ]] && break
    sleep 0.05
done
[[ "$screen" == *MUXY_PHASE7* ]] || fail "read-screen did not observe sent text"
expect_ok "$(accept rename-pane rename-pane --pane "$gamma_pane" CompatPane)"
panes="$(accept list-panes list-panes)"
printf '%s\n' "$panes" | grep -Fq "$gamma_pane" || fail "split pane missing from list-panes"
expect_ok "$(accept close-pane close-pane --pane "$alpha_pane")"
expect_ok "$(accept close-pane close-pane --pane "$shared_pane")"
expect_ok "$(accept close-pane close-pane --pane "$gamma_pane")"

expect_ok "$(accept tab-rename tab rename "$active_tab" CompatTab)"
expect_ok "$(accept tab-set-color tab set-color "$active_tab" blue)"
expect_ok "$(accept tab-set-icon tab set-icon "$active_tab" star.fill)"
expect_ok "$(accept tab-pin tab pin "$active_tab")"
expect_ok "$(accept tab-unpin tab unpin "$active_tab")"
expect_ok "$(accept tab-move tab move "$active_tab" 0)"
expect_ok "$(accept tab-close tab close "$active_tab")"

run_cli "$ROOT/fixtures/gamma" >/dev/null || fail "path-open wrapper invocation failed"
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
printf 'staged app: %s\n' "$APP"
printf 'development socket mode: 0600\n'
printf 'accepted wire heads: 33\n'
printf 'path-open: passed\n'
printf 'raw hook: passed\n'
printf 'raw notification: passed\n'
printf 'production socket preserved: %s pid %s\n' "$PRODUCTION_IDENTITY" "$PRODUCTION_PID"
printf 'normal staged shutdown: passed\n'
