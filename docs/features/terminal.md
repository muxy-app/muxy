# Terminal

Muxy's terminals run in the [server](server.md), powered by
[Ghostty](https://github.com/ghostty-org/ghostty). They keep running after you
quit the app, and every app shows the same screen and history.

## Tabs and splits

| Action | Default shortcut |
| --- | --- |
| New tab | `Cmd+T` |
| New tab in Home | `Cmd+N` |
| Split right / down | `Cmd+D` / `Cmd+Shift+D` |
| Move focus between panes | `Cmd+Opt+Arrows` |
| Zoom the pane | `Cmd+Shift+Return` |
| Close pane / tab | `Cmd+W` / `Cmd+Shift+W` |
| Next / previous tab | `Cmd+]` / `Cmd+[` |
| Go to tab 1–9 | `Cmd+1`–`Cmd+9` |

- New panes open in the project folder. Set **Settings → Terminal → New pane
  directory** to open them in the current pane's folder instead.
- Drag dividers to resize. `Cmd`-drag a pane onto another pane to swap them, or
  onto an edge to dock it there.
- Right-click a tab to rename, color, or pin it. Pinned tabs stay first and
  can't be closed until unpinned.

See [Keyboard shortcuts](../user-guide/keyboard-shortcuts.md) for the full
list.

## Closing and detaching

- Closing a pane ends its terminal, unless another pane in any app still shows
  it.
- Muxy asks first when a program other than the shell is running.
- **Detach Terminal** in the pane's right-click menu removes the pane but keeps
  the terminal running.
- **Existing Terminals** (`Cmd+Opt+T`, or the project menu) lists the project's
  terminals that this app isn't showing, with the app that owns each, and opens
  one in a new tab.
- Set **Settings → General → When closing tabs or panes** to detach to make
  every close a detach.
- Quitting the app or closing its window never ends a terminal. **End All
  Sessions and Quit** in the app menu ends every terminal on this computer,
  without asking first.

## Find

`Cmd+F` searches the focused terminal, including all of its history. `Return`
or `Cmd+G` goes to the next match, `Cmd+Shift+G` to the previous. Search is
plain text and case-insensitive; **Aa** makes it case-sensitive.

## Copy and paste

- `Cmd+C` copies the selection. `Ctrl+C` goes to the program.
- Turn on **Settings → Terminal → Copy on select** to copy when you select.
- When a program uses the mouse, hold `Shift` to select text or open the
  right-click menu.
- Pasting an image into a local terminal sends `Ctrl+V`, so AI tools can read
  it from the clipboard.

## Links and files

- `Cmd`-click a URL or file path to open it. Paths with `:line:column` jump to
  that line in supported editors.
- **Settings → General → Open files with** picks the app for files.
- Dropping files on a terminal pastes their paths.
- On a [remote server](remote-servers.md), dropped and pasted files are
  uploaded first and the remote path is pasted. `Cmd`-clicking a remote file
  offers a read-only copy.

## Shell integration

With [shell integration](server.md#shell-integration), Muxy tracks each
terminal's folder, and you can:

- Jump between prompts with `Cmd+Up` and `Cmd+Down`.
- Select a command's output with **Select Command Output** in the Edit menu or
  the right-click menu.

## Quick Terminal

A terminal that drops down from the top of the screen from anywhere. Record a
global shortcut in **Settings → Quick Terminal** first; none is set by default.

It keeps one terminal in Home, hides when it loses focus, and has its own size
and transparency settings. `Cmd+W` ends its terminal.

## Appearance and configuration

Fonts, transparency, vibrancy, cursor, padding, selection, and scrolling have native controls
in **Settings → Terminal**. Themes are in **Settings → Appearance**. Existing
terminal key bindings are migrated automatically. See
[Settings](../user-guide/settings.md#terminal-configuration).

- `Cmd+=` and `Cmd+-` change the focused pane's font size. `Cmd+0` resets it.
- `Option` acts as `Alt` by default (configurable in Terminal settings).
- Images use the Kitty graphics protocol. Sixel and iTerm2 images are not
  supported.
