# Terminal UI

Run `muxy` with no arguments to open Muxy inside any terminal. It shows the
same projects and terminals as the desktop app, with its own tabs and splits.
It works on macOS and Linux, locally or over SSH.

```bash
muxy                   # this computer's server
muxy --host devbox     # the server on another computer, over SSH
```

## Keys

Every command starts with the prefix `Ctrl-B`, like tmux. Press `Ctrl-B ?` for
help.

| After `Ctrl-B` | Action |
| --- | --- |
| `c` | New tab in the project folder |
| `n` / `p` | Next / previous tab |
| `0`–`9` | Go to tab |
| `%` | Split side by side |
| `"` | Split top and bottom |
| Arrows | Move focus between panes |
| `Ctrl` + Arrows | Resize the split |
| `z` | Zoom the pane |
| `x` | Close the pane |
| `s` | Switch project |
| `w` | Open an existing terminal of this project |
| `d` | Detach and quit; terminals keep running |
| `Ctrl-B` | Send `Ctrl-B` to the terminal |

## Behavior

- The first run opens one shell in Home. After that, it restores your layout.
- Several `muxy` windows share one layout, and changes in one show in the
  others.
- Quitting or detaching never ends a terminal. Closing a pane ends its terminal
  unless another app still shows it, and asks first if a program is running.
- With `--host`, the layout for that server is kept on this computer.
- A tab holds up to 16 panes, and a project up to 64 tabs.

## Not supported

The terminal UI has no mouse support, copy mode, scrollback keys, custom keys,
AI agent status, or [project layouts](../layouts/overview.md). Images in the
terminal are not shown. Desktop and terminal UI layouts are kept separately.
