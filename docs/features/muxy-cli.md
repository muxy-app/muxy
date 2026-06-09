# Muxy CLI

The `muxy` command lets you open projects and control Muxy workspaces from a terminal or automation script.

Use it for quick project launching, switching projects or worktrees, scripted split layouts, tab navigation, sending input to panes, reading visible terminal output, and closing or renaming panes without switching back to the UI.

## Install

Install the CLI from **Muxy → Install CLI**.

Muxy first tries to install `muxy` to `/usr/local/bin/muxy`. If that needs admin access, macOS prompts for permission. If installation there fails, Muxy falls back to `~/bin/muxy` or `~/.local/bin/muxy`.

After installing, verify it is on your `PATH`:

```bash
muxy --help
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

Refresh worktrees from Git:

```bash
muxy refresh-worktrees
muxy refresh-worktrees "My App"
```

## Pane control

Pane-control commands talk to the running Muxy app through a local Unix socket. Muxy must be open.

### Create splits

When you run `muxy split-right` or `muxy split-down` from inside a Muxy pane, the split starts from the pane you are in. Muxy exports `MUXY_PANE_ID` into every pane, and `muxy` uses it automatically.

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

Move through tabs:

```bash
muxy next-tab
muxy previous-tab
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

The socket is private to your user. It does not grant extra privileges, but any process already running as your user can use it while Muxy is open to:

- list and switch projects
- list, switch, or refresh worktrees
- list, switch, or create tabs
- list panes
- read visible terminal text
- send text or supported control keys
- rename or close panes
- create new splits

Avoid exposing sensitive terminal output if you are running untrusted local software.

## Troubleshooting

If `muxy` is not found, make sure its install directory is on your `PATH`.

If pane commands fail with `Muxy is not running`, open Muxy and try again.

If a command with spaces or shell operators is not behaving as expected, quote it:

```bash
muxy split-right "echo a | wc"
```
