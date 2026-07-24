# Muxy CLI

The `muxy` command lets you open projects and control Muxy workspaces from a terminal or automation script.

Use it for quick project launching, switching projects or worktrees, scripted split layouts, tab navigation, browser automation, sending input to panes, reading visible terminal output, and closing or renaming panes without switching back to the UI.

## Install

Install the CLI from **Muxy → Install CLI**.

Muxy first tries to install `muxy` to `/usr/local/bin/muxy`. If that needs admin access, macOS prompts for permission. If installation there fails, Muxy falls back to `~/bin/muxy` or `~/.local/bin/muxy`.

After installing, verify it is on your `PATH`:

```bash
muxy --help
```

Run `muxy <command> --help` (or `-h`) to see the options for a single command:

```bash
muxy create-worktree --help
```

## Open a project

Open the current folder:

```bash
muxy .
```

Open a specific folder:

```bash
muxy ~/Developer/my-app
```

If the project is already open, Muxy selects the existing project instead of creating a duplicate.

## Project and worktree control

Project and worktree commands talk to the running Muxy app through a local Unix socket. Muxy must be open.

### List and switch projects

List projects:

```bash
muxy list-projects
```

Output is tab-separated:

```text
<project-id>  <name>  <path>  <active>
```

Switch to a project by name, ID, or path:

```bash
muxy switch-project "My App"
muxy switch-project ~/Developer/my-app
```

### List and switch worktrees

List worktrees for the active project:

```bash
muxy list-worktrees
```

List worktrees for a specific project:

```bash
muxy list-worktrees "My App"
```

Output is tab-separated:

```text
<worktree-id>  <name>  <path>  <branch>  <active>
```

Switch to a worktree by name, ID, path, or branch:

```bash
muxy switch-worktree feature/login
muxy switch-worktree "Feature Login"
```

Switch to a worktree in a specific project:

```bash
muxy switch-worktree "Feature Login" --project "My App"
```

### Create a worktree

Create a worktree and switch to it. By default the branch matches the name and is created fresh:

```bash
muxy create-worktree login
```

Options:

- `--branch <branch>` — branch to create or check out (defaults to `<name>`)
- `--base <branch>` — base the new branch on this branch instead of the current `HEAD`
- `--existing` — check out an existing branch instead of creating one
- `--path <path>` — place the worktree at a specific path
- `--project <name|id|path>` — target a project other than the active one

When `--path` is omitted, Muxy uses the project's worktree path template or folder setting. Template variables use the
requested `--branch`, including when the branch differs from the worktree name. Templates must include `{branch}`. An
explicit `--path` always wins.

```bash
muxy create-worktree login --branch feature/login --base main
muxy create-worktree hotfix --existing --branch release/1.2
muxy create-worktree review --project "My App" --path ~/worktrees/review
```

On success it prints `ok`, the worktree ID, name, path, and branch (tab-separated).

Refresh worktrees from Git:

```bash
muxy refresh-worktrees
muxy refresh-worktrees "My App"
```

## Pane control

Pane-control commands talk to the running Muxy app through a local Unix socket. Muxy must be open.

### Create splits

When you run `muxy split-right` or `muxy split-down` from inside a Muxy pane, the split starts from the pane you are in. Muxy exports `MUXY_PANE_ID` into every pane, and `muxy` uses it automatically. The new pane belongs to the same top-level tab as its source, including when the source is already a child pane.

Split the current pane to the right:

```bash
muxy split-right
```

Split the current pane downward:

```bash
muxy split-down
```

Create a split and run a command in the new pane:

```bash
muxy split-right npm run dev
muxy split-down "echo a | wc"
```

Both commands print the new pane ID. Save it when you want to control that pane later:

```bash
PANE=$(muxy split-right npm run dev)
```

Split from a different pane with `--from`:

```bash
muxy split-right --from "$PANE" "npm test"
```

`split-right` and `split-down` also accept `--project` and `--worktree` to split a pane in another worktree's workspace without leaving your current view. See [Targeting a project or worktree](#targeting-a-project-or-worktree).

### List panes

```bash
muxy list-panes
```

Output is tab-separated:

```text
<pane-id>  <title>  <cwd>  <focused>
```

Example:

```bash
muxy list-panes | column -t -s $'\t'
```

### Send text

Send text to a pane:

```bash
muxy send --pane "$PANE" "npm test"
```

Send text and press Enter:

```bash
muxy send --pane "$PANE" "npm test"
muxy send-keys --pane "$PANE" Enter
```

Supported keys:

- `Escape` or `Esc`
- `Enter` or `Return`
- `Tab`
- `Ctrl+C` or `Ctrl-C`
- `Ctrl+D` or `Ctrl-D`
- `Ctrl+Z` or `Ctrl-Z`
- `Backspace`

### Read screen content

Read the last 50 visible lines:

```bash
muxy read-screen --pane "$PANE"
```

Read a specific number of lines:

```bash
muxy read-screen --pane "$PANE" --lines 20
```

This reads visible terminal cells, not the full scrollback history.

### Rename and close panes

Rename a pane tab:

```bash
muxy rename-pane --pane "$PANE" "Dev Server"
```

Close a pane:

```bash
muxy close-pane --pane "$PANE"
```

## Tab control

Tab commands talk to the running Muxy app through a local Unix socket. Muxy must be open.

### List tabs

```bash
muxy list-tabs
```

Output is tab-separated:

```text
<index>  <tab-id>  <kind>  <title>  <active>
```

Add `--project` or `--worktree` to list the tabs of another worktree. See [Targeting a project or worktree](#targeting-a-project-or-worktree).

`list-tabs` deliberately stays flat: it includes top-level tabs and their split-child tabs in one indexed list. Switching to a child entry activates its owning top-level tab and focuses that pane.

`muxy tab move <tab> <to-index>` uses this flat list for both arguments. A top-level tab can move to another top-level slot, while a split-child tab can move only to a slot in its current pane.

### Switch and create tabs

Switch tabs by index, ID, or title:

```bash
muxy switch-tab 0
muxy switch-tab "Server Logs"
```

Create a new terminal tab:

```bash
muxy new-tab
```

`new-tab`, `switch-tab`, and the navigation commands take `--project` and `--worktree` to act on another worktree; `new-tab` returns the new tab's ID so you can address it later. See [Targeting a project or worktree](#targeting-a-project-or-worktree).

Move through tabs:

```bash
muxy next-tab
muxy previous-tab
```

### Targeting a project or worktree

`new-tab`, `list-tabs`, `switch-tab`, `next-tab`, `previous-tab`, `split-right`, `split-down`, `browser open`, and `browser list` accept two optional flags to choose where the action runs:

- `--project <name|id|path>` — target a specific project
- `--worktree <name|id|branch>` — target a specific worktree

Resolution rules (both flags are optional):

- Neither flag: acts on the active worktree, exactly as before.
- `--worktree` only: resolves the worktree in the active project; if it is not there, Muxy searches all projects for a unique match by name, branch, or ID. If the match is ambiguous across projects, the command errors — pass `--project` to disambiguate.
- `--project` only: targets that project's active worktree.
- Both: targets the worktree in the given project explicitly.

Targeting a worktree does not move or switch your visible workspace. The action runs in the target worktree's workspace in the background, even when it belongs to a different project than the one you are looking at, and the created tab or browser tab is addressable by its returned ID. To actually move your view, run `switch-project` or `switch-worktree`. This keeps your focus where you expect it.

```bash
muxy new-tab --worktree feature/login
muxy list-tabs --project "My App" --worktree main
muxy browser open localhost:3000 --worktree feature/login
muxy split-right npm test --worktree feature/login
```

## Browser control

Browser commands talk to built-in browser tabs in the running Muxy app. Browser automation that evaluates JavaScript or interacts with the DOM requires the target tab to be open and rendered.

Open, list, read, navigate, and close browser tabs:

```bash
TAB=$(muxy browser open "https://example.com" --split)
muxy browser list
muxy browser read "$TAB"
muxy browser navigate "$TAB" "https://muxy.app"
muxy browser close "$TAB"
```

`browser open` and `browser list` accept `--project` and `--worktree` to open or list browser tabs in another worktree's workspace. The opened tab's ID still comes back, so you can read or automate it without switching views. See [Targeting a project or worktree](#targeting-a-project-or-worktree).

Automate the page:

```bash
muxy browser wait-for "$TAB" "input[name=q]" 5000
muxy browser type "$TAB" "input[name=q]" "muxy" --submit
muxy browser wait-for-navigation "$TAB" 10000
muxy browser eval "$TAB" "document.title"
```

Supported browser subcommands are:

```text
open, navigate, list, read, close, eval, click, type, fill, press, select,
hover, check, uncheck, scroll-into-view, wait, wait-for,
wait-for-navigation, get-text, get-html, get-value, get-attribute,
get-count, is, find, snapshot, reload, back, forward, screenshot,
storage, cookies
```

## Example workflow

Create a small development layout:

```bash
WEB=$(muxy split-right npm run dev)
TESTS=$(muxy split-down --from "$WEB" npm test)

muxy rename-pane --pane "$WEB" "Web"
muxy rename-pane --pane "$TESTS" "Tests"
```

Run a command in the tests pane later:

```bash
muxy send --pane "$TESTS" "npm test -- --watch"
muxy send-keys --pane "$TESTS" Enter
```

## Security model

Pane control is local to your macOS user account.

Muxy listens on:

```text
~/Library/Application Support/Muxy/muxy.sock
```

Override the socket location with the `MUXY_SOCKET_PATH` environment variable if you run Muxy with a non-default socket.

The socket is private to your user. It does not grant extra privileges, but any process already running as your user can use it while Muxy is open to:

- list and switch projects
- list, switch, create, or refresh worktrees
- list, switch, or create tabs
- open, navigate, read, automate, screenshot, and close built-in browser tabs
- read and write browser local/session storage and cookies
- list panes
- read visible terminal text
- send text or supported control keys
- rename or close panes
- create new splits

Avoid exposing sensitive terminal output if you are running untrusted local software.

## Install skills

Install both Muxy agent skills into every detected AI harness:

```bash
muxy install-skills
```

Additional arguments are forwarded to `skills add`.

## Troubleshooting

If `muxy` is not found, make sure its install directory is on your `PATH`.

If pane commands fail with `Muxy is not running`, open Muxy and try again.

If a command with spaces or shell operators is not behaving as expected, quote it:

```bash
muxy split-right "echo a | wc"
```

## Skill for AI agents

Muxy ships a `muxy-cli` skill that teaches a coding agent when and how to drive the workspace with these commands — capturing pane IDs, sending input safely, and reading the screen. Install it into a project with [skills.sh](https://www.skills.sh):

```bash
npx skills add github.com/muxy-app/muxy/tree/main/Muxy/Resources/skills/muxy-cli
```

Muxy's companion `muxy-extension` skill (for building extensions) installs the same way:

```bash
npx skills add github.com/muxy-app/muxy/tree/main/Muxy/Resources/skills/muxy-extension
```

The skill source is [`Muxy/Resources/skills/muxy-cli/SKILL.md`](https://github.com/muxy-app/muxy/blob/main/Muxy/Resources/skills/muxy-cli/SKILL.md).
