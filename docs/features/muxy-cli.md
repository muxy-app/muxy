# Muxy CLI

The `muxy` command talks to the [server](server.md) from a shell. It manages
projects, terminal sessions, worktrees, server settings, and phone access. No
desktop app is needed, so it works on Linux and over SSH too.

Commands work with what the server owns: projects and sessions. Tabs, panes, and
workspaces belong to each app, so there are no commands for them. Browser tabs
are the exception: they live in the desktop app, and `muxy browser` talks to
the app on this computer. The [terminal UI](terminal-ui.md) is an app of its
own, with its own layout.

## Install

- **With the desktop app:** choose **Install Command Line Tool…** from the app
  menu. It links `~/.local/bin/muxy` to the copy inside the app.
- **On its own:** see [Getting started](../user-guide/getting-started.md#cli-and-server-only).

Make sure `~/.local/bin` is on your `PATH`.

## Basics

```bash
muxy                 # open the terminal UI
muxy .               # open this folder in the desktop app or the terminal UI
muxy project list    # list projects
muxy --help          # all commands; muxy <command> --help for details
```

- Commands start the server they use if it isn't running, including a remote
  one with `--host`. `muxy server status` and `muxy server stop` never start
  it.
- Management commands take `--json` for machine-readable output, except
  `muxy mobile`.
- Ending or discarding a session and deleting a project or worktree need
  `--yes`.
- A project can be named by its ID, its name if unique, or its folder.
- `--host <destination>` runs the command against the server on another
  computer. It must come first: `muxy --host devbox project list`. See
  [Remote servers](remote-servers.md).

## Commands

| Command | What it does |
| --- | --- |
| `muxy` | Opens the [terminal UI](terminal-ui.md). |
| `muxy <folder>` | Opens a new terminal in the folder's project in the desktop app, adding the project if needed. Over SSH, or without the desktop app, opens it in the terminal UI. |
| `muxy server start\|status\|stop` | Starts, inspects, or stops the server. `stop` refuses while terminals run; add `--force` to end them. |
| `muxy project list\|add\|rename\|set-color\|set-icon\|delete` | Manages projects. Deleting ends its terminals but keeps files on disk. |
| `muxy session list\|create\|send\|send-keys\|read-screen\|history\|search\|wait\|end\|discard\|show` | Creates terminals, sends input, reads output, waits for text or exit, and shows a terminal in the desktop app. |
| `muxy worktree list\|create\|checkout-pr\|register\|remove` | Manages Git worktree projects, including checking out pull requests. |
| `muxy settings get\|set` | Reads or changes [server settings](server.md#settings). |
| `muxy config list\|get\|set\|unset\|reload` | Reads or changes the desktop app's settings on this computer and applies them to the running app. See [App settings](#app-settings). |
| `muxy activity list\|ack` | Reads or clears [AI agent](ai-agents.md) activity. |
| `muxy browser open\|list\|read\|click\|…` | Drives browser tabs in the desktop app on this computer. See [Browser control](#browser-control). |
| `muxy extension list\|status\|logs\|load\|enable\|disable\|reload\|unload\|validate\|publish` | Manages extensions in the desktop app on this computer. See [Extension commands](#extension-commands). |
| `muxy status <state> [--app APP] [--msg TEXT]` | Tells the terminal what a script is doing (OSC 7501): `idle`, `working`, `done`, `blocked`, `error`, or `clear`. Needs no server. |
| `muxy exec <project> -- <program> [args]` | Runs a program in the project folder, without a shell. |
| `muxy git`, `muxy files` | Run raw server Git and file actions as JSON. Advanced; the JSON shape follows the protocol and may change. |
| `muxy mobile` | Turns on phone access, pairs phones, and turns push notifications on or off with `muxy mobile push on\|off`. See [Mobile](mobile.md). |
| `muxy install-skills` | Installs the Muxy CLI skill for the AI agents found in your home folder. `--dir <path>` adds another skills folder. |
| `muxy stdio` | Joins stdin and stdout to the server. Used by `--host` over SSH; you don't run it yourself. |

## Extension commands

These talk to the desktop app on this computer and start it if needed, so they
don't take `--host`. [Agents building extensions](../extensions/agents.md) use
them too.

| Command | What it does |
| --- | --- |
| `muxy extension list [--json]` | Lists extensions with their version, state, source, and folder, then any load errors. |
| `muxy extension status <name> [--json]` | Shows one extension, its load errors, and recent log lines. |
| `muxy extension logs <name> [--lines N]` | Prints recent log lines. The default is 100. |
| `muxy extension load <folder>` | Loads an unpacked folder, disabled. Loading a folder that is already loaded reloads it. |
| `muxy extension enable <name>`, `disable <name>` | Turns an extension on or off. |
| `muxy extension reload <name>` | Restarts one extension from its folder. If the folder no longer loads, or its package name changed, it prints why and the running copy stays. |
| `muxy extension unload <name>` | Forgets an unpacked folder and keeps its files. It doesn't work on marketplace extensions. |
| `muxy extension validate <name\|folder> [--json]` | Checks what publishing needs, including the GitHub CLI, and exits nonzero if something is missing. |
| `muxy extension publish <name> [--json]` | Opens a [pull request to the store](../extensions/contributing.md#publish-from-muxy), after you approve it in the app. |

## App settings

```bash
muxy config list                          # every setting, defaults included
muxy config get terminal                  # one table, as TOML
muxy config set appearance.dark_theme "Builtin Dark"
muxy config set terminal.font_size 14
muxy config set 'keymap."settings.focus_navbar"' cmd-l
muxy config unset terminal.font_size
muxy config reload                        # apply hand edits
```

- Keys are dotted paths in `settings.toml`, or in `terminal.toml` after
  `terminal.`. Quote a part that holds a dot. New settings get keys
  automatically, from the files' own format. Optional settings appear in
  `list` once they are set.
- Values are TOML, such as `true`, `14`, or `["Menlo", "Monaco"]`. Anything else
  is text, so names need no quotes of their own.
- A change is saved only if the app can still start with it, and the file is
  left as it was otherwise. `set` and `unset` then have a running desktop app
  apply it. They never start the app, which reads the files when it starts.
- Remote servers are read-only here; manage them in **Settings → Devices**.
  Server settings use `muxy settings`.
- `config` works on this computer's profile and doesn't take `--host`. Every
  command takes `--json` for machine-readable output.

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
- `show` opens a session in a new tab of its project in the desktop app on
  this computer, starting it if needed. When a tab already shows the session,
  `show` selects that tab instead. It never brings a running app to the front.
  With `--background`, `show` adds the tab without switching to it.
- Inside a Muxy terminal, session commands default to that terminal, and
  `create` to its project.
- `create` can take a `--directory` and a command to run after `--`.
- `wait --text <text>` waits for text on the screen, and `wait --exit` for the
  shell to exit. Both give up after 30 seconds by default.

## Browser control

```bash
tab=$(muxy browser open localhost:3000)
muxy browser wait-for-navigation "$tab"
muxy browser fill "$tab" "#email" "me@example.com"
muxy browser click "$tab" "button[type=submit]"
muxy browser wait "$tab" --text "Welcome"
muxy browser read "$tab"
muxy browser screenshot "$tab" | base64 -D > page.png
muxy browser close "$tab"
```

- Browser commands need the desktop app and don't take `--host`. `open` starts
  the app if it isn't running.
- `open` prints the new tab's ID. Pass it to the other commands.
- `snapshot` lists a page's interactive elements, each with a selector for
  `click`, `fill`, or `type`.
- `muxy browser --help` lists every command.

## Exit codes

`0` on success. On any error, `muxy` prints `muxy: <message>` to stderr and exits
with `1`, including when a program run with `muxy exec` fails.

## Profiles

`MUXY_DIR` points the CLI, and the server it starts, at another profile folder.
Use it to run a separate, throwaway server. See [Server](server.md#files).
