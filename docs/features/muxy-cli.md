# Muxy CLI

The `muxy` command talks to the [server](server.md) from a shell. It manages
projects, terminal sessions, worktrees, server settings, and phone access. No
desktop app is needed, so it works on Linux and over SSH too.

The CLI works with what the server owns: projects and sessions. Tabs, panes, and
workspaces belong to each app, so the CLI never changes them.

## Install

- **With the desktop app:** choose **Install Command Line Tool…** from the app
  menu. It links `~/.local/bin/muxy` to the copy inside the app.
- **On its own:** see [Getting started](../user-guide/getting-started.md#cli-and-server-only).

Make sure `~/.local/bin` is on your `PATH`.

## Basics

```bash
muxy                 # open the terminal UI
muxy .               # open this folder in the desktop app (macOS)
muxy project list    # list projects
muxy --help          # all commands; muxy <command> --help for details
```

- Commands start the server they use if it isn't running, including a remote
  one with `--host`. `muxy server status` and `muxy server stop` never start
  it.
- Add `--json` to any management command for machine-readable output.
- Commands that end terminals or delete data need `--yes`.
- A project can be named by its ID, its name if unique, or its folder.
- `--host <destination>` runs the command against the server on another
  computer. It must come first: `muxy --host devbox project list`. See
  [Remote servers](remote-servers.md).

## Commands

| Command | What it does |
| --- | --- |
| `muxy` | Opens the [terminal UI](terminal-ui.md). |
| `muxy <folder>` | Opens the folder as a project in the desktop app. macOS only. |
| `muxy server start\|status\|stop` | Starts, inspects, or stops the server. `stop` refuses while terminals run; add `--force` to end them. |
| `muxy project list\|add\|rename\|set-color\|set-icon\|delete` | Manages projects. Deleting ends its terminals but keeps files on disk. |
| `muxy session list\|create\|send\|send-keys\|read-screen\|history\|search\|end\|discard` | Creates terminals, sends input, and reads output. |
| `muxy worktree list\|create\|register\|remove` | Manages Git worktree projects. |
| `muxy settings get\|set` | Reads or changes [server settings](server.md#settings). |
| `muxy activity list\|ack` | Reads or clears [AI agent](ai-agents.md) activity. |
| `muxy exec <project> -- <program> [args]` | Runs a program in the project folder, without a shell. |
| `muxy git`, `muxy files` | Run raw server Git and file actions as JSON. Advanced; the JSON shape follows the protocol and may change. |
| `muxy mobile` | Turns on phone access and pairs phones. See [Mobile](mobile.md). |
| `muxy stdio` | Joins stdin and stdout to the server. Used by `--host` over SSH; you don't run it yourself. |

## Sessions from a script

```bash
id=$(muxy session create my-app)
muxy session send "$id" "npm test"
muxy session send-keys "$id" Enter
muxy session read-screen "$id" --lines 20
muxy session end "$id" --yes
```

- `send` types text without pressing Return. `send-keys` sends `Enter`, `Tab`,
  `Escape`, `Backspace`, `Ctrl+C`, `Ctrl+D`, or `Ctrl+Z`.
- `read-screen` returns the last visible rows. `history` and `search` page
  through scrollback as JSON.
- `--saved` reads a session's last saved screen, even after it ended.
- `end` stops the program and keeps its saved output. `discard` also deletes
  the output.
- Sessions created here outlive the command and appear in every app under
  **Existing Terminals**.

## Exit codes

`0` on success. On any error, `muxy` prints `muxy: <message>` to stderr and exits
with `1`, including when a program run with `muxy exec` fails.

## Profiles

`MUXY_DIR` points the CLI, and the server it starts, at another profile folder.
Use it to run a separate, throwaway server. See [Server](server.md#files).
