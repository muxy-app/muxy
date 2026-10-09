---
name: muxy-cli
description: How to drive Muxy 2 from a shell with the `muxy` command — start terminals that keep running, type into them, read and wait on their output, and manage projects and Git worktrees. Use this when you are an agent running inside a Muxy terminal (or any script on the same computer) and want work to run in its own terminal instead of blocking yours.
---

# Muxy CLI

Muxy keeps terminals in a background server. The `muxy` command talks to that
server directly; the desktop app doesn't need to be open. Terminals you start
keep running after your command returns, and the user can open them in the
desktop app or the terminal UI from **Existing Terminals**.

Run `muxy --help` and `muxy <command> --help` for every option. Add `--json`
to any management command for machine-readable output.

## When to use it

- Start a long-running process (a dev server, a test watcher, a log tail) in its
  own terminal, then check on it later.
- Wait until that process prints something, or exits, before going on.
- Create a Git worktree for a task, with the user's configured location.

Don't use it for work a plain command can do in your own terminal.

## Inside a Muxy terminal

Muxy sets `MUXY_SESSION_ID` and `MUXY_SERVER_ID` in every terminal. Session
commands then default to your own terminal, and `session create` to your
project, so most commands need no IDs.

## Start a terminal and capture its ID

`session create` prints the new session ID. Everything after `--` is typed into
the new terminal, followed by Return. Capture the ID; never guess one.

```bash
WEB=$(muxy session create -- npm run dev)
TESTS=$(muxy session create MyProject --directory ./packages/api -- npm test)
```

Outside a Muxy terminal, name the project: an ID, a unique name, or its folder.

The words after `--` are joined with spaces, so quote the whole command to keep
its own quotes, pipes, or `;`:

```bash
muxy session create -- 'grep -rn "TODO list" src | wc -l'
```

## Wait instead of polling

```bash
muxy session wait "$WEB" --text "ready" --timeout-ms 120000   # prints the matching line
muxy session wait "$TESTS" --exit                              # prints how it ended
```

`wait` fails with a nonzero exit if the timeout passes (default 30 s) or, for
`--text`, if the terminal ends without showing the text. It looks at the
visible screen, not scrollback. Don't wait on your own terminal: it is busy
running `muxy`.

## Read output

```bash
muxy session read-screen "$WEB" --lines 40     # last visible rows, not scrollback
muxy session search "$WEB" "error" --ignore-case
muxy session history "$WEB" --limit 200        # JSON, paged with --before
```

After a terminal ends, add `--saved` to read its last saved output.

## Send input deliberately

`send` types text without pressing Return; `send-keys` presses one key
(Enter, Tab, Escape, Backspace, Ctrl+C, Ctrl+D, Ctrl+Z).

```bash
muxy session send "$TESTS" "npm test -- --watch"
muxy session send-keys "$TESTS" Enter
muxy session send-keys "$WEB" Ctrl+C
```

You are typing into a live terminal. Read the screen first if you are unsure
what is running.

## End terminals you started

```bash
muxy session list --project MyProject     # id, project-id, status, attached, directory
muxy session end "$WEB" --yes
```

Only end terminals you created. A terminal the user is looking at closes in
their app too.

## Projects and worktrees

```bash
muxy project list
muxy project add ~/code/app --reuse                 # prints the existing project if there is one
muxy worktree create MyProject login                # branch "login" from HEAD, default location
muxy worktree create MyProject fix --branch fix/x --base main --hooks
muxy worktree list MyProject
muxy worktree remove login --yes
```

`--hooks` runs the setup or teardown commands from `.muxy/worktree.json`; ask
the user before using it. `worktree remove` refuses uncommitted changes unless
`--force` is given; never pass `--force` without the user's consent.

## Other computers

Put `--host user@computer` before any command to use the server on another
computer over SSH. Directories are then absolute paths on that computer.
