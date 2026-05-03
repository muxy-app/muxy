# Terminal

Muxy's terminals are powered by [libghostty](https://github.com/ghostty-org/ghostty), running on a Metal layer for fast, GPU‑accelerated rendering.

## Configuration

Ghostty is configured via `~/.config/ghostty/config`. Open it with **Muxy → Open Configuration…** Reload after editing with **Muxy → Reload Configuration** (`Cmd+Shift+R`).

Most Ghostty options work — fonts, colors, padding, keybinds, shell integration. Muxy applies the active light/dark variant automatically when the system appearance changes.

## Find in terminal

`Cmd+F` opens an inline search overlay scoped to the focused terminal pane. Enter / Shift‑Enter cycle through matches; Escape dismisses.

## Copy and paste

- **Copy:** `Cmd+C` while text is selected. With nothing selected, `Cmd+C` is sent to the running program (so `Ctrl+C`‑style apps still work via their own bindings).
- **Paste:** `Cmd+V`, or right‑click → Paste.
- Middle‑click pastes the X11 selection if the source supports it.

## Working directory

Muxy tracks the current directory using Ghostty's shell integration (OSC 7). The directory is persisted in workspace snapshots so newly recreated tabs land in the same folder when applicable.

## Custom command shortcuts

You can define reusable shell command shortcuts in **Settings → Keyboard Shortcuts → Custom Commands**:

- Display name, command, optional icon, optional keybinding.
- Triggering one creates a new tab and runs the command in it.
- Useful for `npm run dev`, `make watch`, `just test`, etc.

## Right‑click menu

Inside a terminal pane:

- **Paste**
- **Split Right** / **Split Down**
- **Close Pane**

## Notifications from the terminal

OSC 9 and OSC 777 notification escape sequences are routed into Muxy's notification panel and (optionally) macOS notifications. See [Notifications](notifications.md).

## Quick‑select labels

Ghostty's quick‑select feature lets you focus a pane or surface by typing a label key. Labels and bindings are configured in the Ghostty config.
