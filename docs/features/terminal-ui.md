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
| `1`–`9`, `0` | Go to tabs 1–9, or tab 10 |
| `%` | Split side by side |
| `"` | Split top and bottom |
| Arrows | Move focus between panes |
| `Ctrl` + Arrows | Resize the split |
| `r` | Enter resize mode; use arrows, then `Esc` to leave |
| `o` / `Tab` | Next pane |
| `z` | Zoom the pane |
| `x` | Close the pane |
| `s` | Switch project |
| `(` / `)` | Previous / next project |
| `b` | Toggle the sidebar |
| `m` | Toggle mouse support |
| `[` | Enter scrollback |
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
- The sidebar shows AI agent activity.

## Mouse and scrollback

Click to focus a pane or pick a project or tab, and drag a divider to resize.
Drag across text to select it; it is copied on release. Double-click selects a
word and triple-click a line. Right-click opens a menu. When a program uses the
mouse, hold `Shift` to select.

`Ctrl-B [` or `Ctrl-B PageUp` opens scrollback. Scroll with the arrows,
`PageUp`, and `PageDown`; `q` or `Esc` returns to the live screen. The mouse
wheel scrolls history too, unless the program uses the mouse or fills the
screen; then hold `Shift`.

## Not supported

The terminal UI has no custom key bindings or
[project layouts](../layouts/overview.md), and doesn't show images in the
terminal. Desktop and terminal UI layouts are kept separately.
