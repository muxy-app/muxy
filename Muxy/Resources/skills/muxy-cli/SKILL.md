---
name: muxy-cli
description: How to drive the Muxy macOS terminal multiplexer from a shell with the `muxy` command — open projects, switch projects/worktrees/tabs, build split layouts, send input to panes, and read visible terminal output. Use this when you are an agent running inside a Muxy pane (or any script on the same machine) and want to control the workspace instead of asking the user to click. Mechanics and the security model live in the linked docs.
---

# Muxy CLI Guide

The `muxy` command lets an agent or script control a running Muxy workspace: open projects, switch projects/worktrees/tabs, create split panes, send keystrokes to a pane, and read back what a pane is showing. You drive the same workspace the user sees — treat it as a shared surface, not a private scratchpad.

**This skill is the usage layer — when to reach for the CLI and how to use it safely.** For the full command reference, output formats, and security model, read the docs page (append `/plain` for raw Markdown):

> **<https://muxy.app/docs/features/muxy-cli/plain>**

The LLM-friendly index lists every Muxy docs page: **<https://muxy.app/llms.txt>**.

## When to use it

- **You are running inside a Muxy pane** and want to spawn a sibling pane (a dev server, a test watcher, a logs tail) instead of blocking your own terminal. `muxy split-right`/`split-down` start from *your* pane automatically — Muxy exports `MUXY_PANE_ID` into every pane and the CLI reads it.
- **You need to orchestrate work across panes** — start a process in one pane, then later send it input or read its output by pane ID.
- **You are scripting project setup** — open a folder, switch to the right worktree, lay out a known split arrangement.

Don't use it to do work a plain shell command can do in your own pane. Splitting, sending keys, and reading screens are for when the work genuinely belongs in *another* pane the user is watching.

## First, confirm Muxy is reachable

Every command talks to the running app over a local Unix socket — except opening a path, which falls back to launching Muxy if it is closed. So the socket commands fail with `Error: Muxy is not running` when Muxy is not open, while `muxy <path>` still works. Check once before a sequence of socket commands:

```bash
muxy list-panes >/dev/null 2>&1 || { echo "Muxy not running"; exit 1; }
```

If `muxy` itself is not found, it has not been installed — the user installs it from **Muxy → Install CLI** (it lands in `/usr/local/bin`, or `~/bin` / `~/.local/bin` as a fallback). You cannot install it for them.

`muxy --help` lists every command; `muxy <command> --help` (or `-h`) prints that command's options.

## Capture IDs, never guess them

Pane, tab, project, and worktree commands key off IDs. The split commands **print the new pane ID on stdout** — capture it; do not invent or hardcode IDs.

```bash
WEB=$(muxy split-right npm run dev)
TESTS=$(muxy split-down --from "$WEB" npm test)

muxy rename-pane --pane "$WEB" "Web"
muxy rename-pane --pane "$TESTS" "Tests"
```

For the other surfaces, list and parse the **tab-separated** output (the first column is always the ID/index) rather than matching on a title that may not be unique:

| Command | Columns (tab-separated) |
| --- | --- |
| `muxy list-panes` | `<pane-id>  <title>  <cwd>  <focused>` |
| `muxy list-projects` | `<project-id>  <name>  <path>  <active>` |
| `muxy list-worktrees [project]` | `<worktree-id>  <name>  <path>  <branch>  <active>` |
| `muxy list-tabs` | `<index>  <tab-id>  <kind>  <title>  <active>` |

```bash
PANE=$(muxy list-panes | awk -F'\t' '$2=="Tests"{print $1; exit}')
```

Switch commands resolve a name, ID, path, or branch, so a human-readable argument is fine for `switch-project` / `switch-worktree` / `switch-tab`; capture the ID only when you will address the same pane repeatedly.

## Send input deliberately

`muxy send` types text into a pane **without** pressing Return; `muxy send-keys` presses one supported key. Send the text, then the key — this lets you stage a command and run it in two steps, or send a control key on its own:

```bash
muxy send --pane "$TESTS" "npm test -- --watch"
muxy send-keys --pane "$TESTS" Enter
```

Supported keys: `Escape`/`Esc`, `Enter`/`Return`, `Tab`, `Ctrl+C`/`Ctrl-C`, `Ctrl+D`/`Ctrl-D`, `Ctrl+Z`/`Ctrl-Z`, `Backspace`.

You are typing into a live shell another process owns. Read the screen first if you are unsure what is running, and prefer `Ctrl+C` over assuming a prompt is idle.

## Read the screen, don't scrape scrollback

`muxy read-screen --pane <id> [--lines N]` returns the **last N visible lines** (default 50) of rendered terminal cells — not the full scrollback. Use it to check on a process you started in another pane:

```bash
muxy read-screen --pane "$WEB" --lines 20
```

If you need more history than is on screen, that is a sign the work should write to a file you can read directly, not be scraped from a terminal.

## Worktrees and projects

```bash
muxy switch-project "My App"
muxy switch-worktree feature/login --project "My App"
muxy create-worktree login --branch feature/login --base main
muxy refresh-worktrees
```

`create-worktree <name>` defaults the branch to `<name>` and creates it; pass `--existing` to check out an existing branch, `--base <branch>` to fork from a specific base, and `--path`/`--project` to place or target it. After Git operations done outside Muxy, `refresh-worktrees` re-reads worktrees from Git.

## Tabs

A tab is a whole surface (terminal, source control, an extension) within the active worktree; panes split *inside* a tab. Open one, move between them, or jump straight to a known tab:

```bash
muxy new-tab                 # new terminal tab
muxy switch-tab 0            # by index, ID, or title
muxy switch-tab "Server Logs"
muxy next-tab                # cycle forward
muxy previous-tab            # cycle backward
```

Use `switch-tab` (resolves index/ID/title) when you know the target; reach for `next-tab`/`previous-tab` only for relative cycling. List first with `muxy list-tabs` when you need the index or ID.

## Browser

The built-in browser opens web pages in a tab and can be fully automated for workflows — navigation, DOM interaction, JavaScript, cookies, storage, and screenshots:

```bash
TAB=$(muxy browser open http://localhost:3000)   # returns the browser tab ID
muxy browser open https://example.com --split    # open beside the current pane
muxy browser navigate "$TAB" https://example.com/docs
muxy browser list                                # <tab-id>  <title>  <url>  <profile>  <active>
muxy browser read "$TAB"                          # title, then URL, then visible page text
muxy browser close "$TAB"
```

Automate the page once it is open:

```bash
muxy browser wait "$TAB" --selector "input[name=q]"   # also --text, --url-contains, --function, --timeout-ms
muxy browser wait-for "$TAB" "input[name=q]"          # selector-only shorthand
muxy browser fill "$TAB" "input[name=q]" "muxy"       # set an input's value
muxy browser press "$TAB" Enter                       # dispatch a key
muxy browser wait-for-navigation "$TAB"               # wait for the load to finish
muxy browser click "$TAB" "a.result"
muxy browser select "$TAB" "#region" "us-east"
muxy browser check "$TAB" "#terms"                    # also: uncheck, hover, scroll-into-view

muxy browser eval "$TAB" "document.title"             # run JS, returns the JSON result
muxy browser snapshot "$TAB"                          # visible interactive elements (great for agents)
muxy browser find "$TAB" text "Sign in"               # find by role|text|label|placeholder|testid
muxy browser get-text "$TAB" "h1"                     # also get-html, get-value, get-attribute, get-count
muxy browser is "$TAB" visible "#checkout"            # visible|enabled|checked|disabled|hidden
muxy browser screenshot "$TAB" > page.b64             # base64 PNG of the page
muxy browser reload "$TAB"                            # also: back, forward

muxy browser storage set "$TAB" token abc             # local storage (add 'session' for sessionStorage)
muxy browser storage get "$TAB" token
muxy browser cookies get "$TAB"                       # JSON array of cookies for the tab's profile
muxy browser cookies set "$TAB" session xyz example.com
muxy browser cookies delete "$TAB" session
```

Run `browser open` with no URL to open the configured home page (blank by default). Capture the tab ID from `browser open` (or `browser list`) and reuse it; never guess it. Commands that run JavaScript (`eval`, `click`, `type`, `wait-for`, `get-*`, `screenshot`, `storage`) need the tab open and rendered in the active project — Muxy has no headless browser — so pause after navigating before reading. `navigate`, `cookies`, and `list` work without the tab being visible. Cookies are shared by all tabs on the same profile. If the built-in browser is disabled in Settings, every `browser` command returns an error.

## Install the skills into your AI harnesses

`muxy install-skills` installs the Muxy agent skills (`muxy-cli` and `muxy-extension`) into every AI coding harness it detects on the machine — Claude Code, Codex, Cursor, Droid, Grok, OpenCode, and others — using each tool's own skill location. It wraps `npx skills add` with `--global` (all your projects) and `--yes` (non-interactive), and forwards any extra arguments, so you can scope it like `muxy install-skills -a codex`. Run it once so future sessions of those harnesses pick the skills up automatically; it needs `npx` (Node.js) on `PATH` and does not require Muxy to be running.

## Behavior

- **Quote any command that contains spaces or shell operators** so the whole thing reaches the pane intact: `muxy split-right "echo a | wc"`. An unquoted operator is interpreted by *your* shell, not the new pane.
- **One key per `send-keys`.** It is not a key sequence parser — chain calls for multiple keys.
- **The socket is local to your macOS user.** It grants no extra privileges, but any process running as your user can drive the workspace while Muxy is open. Don't pipe untrusted input into `muxy send`, and be mindful that `read-screen` can surface sensitive output. See the **Security model** section of the docs.
- **Prefer switching to creating.** `switch-project` / `switch-worktree` select an existing entry; opening a path that is already open selects it rather than duplicating it. Reach for `create-worktree` / `new-tab` only when nothing suitable exists.
- **Leave the user's focus where they expect it.** You share the visible workspace — name panes you create (`rename-pane`) so the user can tell what is yours, and close them (`close-pane`) when the work is done.

## Checklist

- [ ] Confirmed Muxy is running before any socket command (path-open is the only exception).
- [ ] Captured every pane ID from the command that created it; never hardcoded one.
- [ ] Parsed list output by the first (ID) column, not by a possibly-duplicate title.
- [ ] Used `send` for text and `send-keys` for one supported key; quoted commands with spaces/operators.
- [ ] Named panes you create and closed them when finished, so the shared workspace stays legible.
