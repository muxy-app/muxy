# Muxy CLI

The `muxy` command lets you open projects and control Muxy panes from a terminal or automation script.

Use it for quick project launching, scripted split layouts, sending input to panes, reading visible terminal output, and closing or renaming panes without switching back to the UI.

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

## Pane control

Pane-control commands talk to the running Muxy app through a local Unix socket. Muxy must be open.

### Create splits

Split the focused pane to the right:

```bash
muxy split-right
```

Split the focused pane downward:

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

Split from a specific pane instead of the focused pane:

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
