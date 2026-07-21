# Terminal

Muxy's terminals are powered by [libghostty](https://github.com/ghostty-org/ghostty), running on a Metal layer for fast, GPU-accelerated rendering.

## Quick terminal

On a display with a camera cutout, hover the cutout for a moment and the terminal expands out of it like a dynamic island. Pressing Shift twice (or a recorded shortcut) opens it from anywhere. It always starts in your home directory and keeps the same shell, working directory, and history while hidden.

Dismiss it with the same shortcut or the close button. Moving the pointer away, clicking another app, and pressing Escape no longer close it, so Escape reaches the terminal for `vim`, `less`, and other full-screen programs. On a display without a camera cutout there is no hover zone; the terminal opens only from the shortcut, at the same top-center position.

System-wide double Shift requires **System Settings → Privacy & Security → Input Monitoring**. You can use **Settings → Shortcuts → Quick Terminal** to enable access or assign a conventional global shortcut that does not require Input Monitoring.

The same settings section controls the terminal width, height, transparency, and background vibrancy. These same controls are also available in-place from the gear button in the quick terminal, applied live to the open panel. Sizes are stored in points, constrained to 480–1200 wide and 280–800 high, and automatically reduced when the active display is smaller. Transparency ranges from 0–55%, and vibrancy uses a continuous 0–100% native material intensity.

Vibrancy controls how much of the native macOS material participates in the background composition. It does not set a custom blur radius, which AppKit does not expose for system materials.

Transparency and vibrancy apply only to the terminal workspace while preserving the active Ghostty theme. The cutout bridge and its controls stay solid for readability, and project terminals keep their own Ghostty configuration. Muxy uses an opaque, unblurred fallback when macOS Reduce Transparency or Increase Contrast is enabled.

The quick terminal is available while Muxy is running. Closing Muxy's main window still follows the existing quit behavior.

## Configuration

Muxy's active Ghostty config is `~/Library/Application Support/Muxy/ghostty.conf`. On first launch Muxy seeds it from `~/.config/ghostty/config` when that file exists; after that, Muxy reads and writes its own copy. Open it with **Muxy -> Open Configuration...**, reload after editing with `⌘⇧R`.

Most Ghostty options work — fonts, colors, padding, keybinds, shell integration. Muxy applies the active light/dark variant automatically when the system appearance changes.

## Find in terminal

`⌘F` opens an inline search overlay scoped to the focused pane. Enter / Shift-Enter cycle through matches; Escape dismisses.

## Copy and paste

| Action | Shortcut |
| --- | --- |
| Copy (with selection) | `⌘C` |
| Send `^C` to program | `⌘C` with no selection |
| Paste | `⌘V` or right-click → Paste |
| X11 selection paste | Middle-click |

Enable **Settings -> Terminal -> Auto-copy terminal selection** to copy selected terminal text on mouse release.

## Working directory

Muxy tracks the cwd via Ghostty's shell integration (OSC 7). The directory is persisted in workspace snapshots so newly recreated tabs land in the same folder when applicable.

Remote terminals use the selected SSH device's environment before starting the remote login shell. New SSH devices default to `TERM=xterm-256color`; edit the device in Settings -> Remote Devices to change or remove it.

## Muxy CLI

Use the `muxy` command to open projects and control panes from a shell or automation script. See [Muxy CLI](muxy-cli.md).

## Custom command shortcuts

Define reusable shell command shortcuts in **Settings → Commands**:

- Display name, command, optional icon, optional keybinding.
- Triggering one creates a new tab and runs the command.
- Useful for `npm run dev`, `make watch`, `just test`, …

## Rich Input

`Cmd+I` opens a multiline composer for prompts, files, images, and broadcast sends.

## Right-click menu

Inside a terminal pane: **Paste**, **Split Right**, **Split Down**, **Close Pane**.

## Notifications from the terminal

OSC 9 and OSC 777 notification escape sequences are routed into Muxy's notification panel and (optionally) macOS notifications.

For AI coding agents (Claude Code, Codex, Cursor, Droid, Grok, OpenCode, Pi), Muxy uses hook-based lifecycle events rather than escape sequences — see [AI notifications](ai-notifications.md).

## Quick-select labels

Ghostty's quick-select feature lets you focus a pane or surface by typing a label key. Labels and bindings are configured in the Ghostty config.
