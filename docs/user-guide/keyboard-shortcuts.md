# Keyboard Shortcuts

Defaults for the desktop app. Change any of them in **Settings → Keyboard**.
For the terminal UI, see [Terminal UI keys](../features/terminal-ui.md#keys).

Most actions here are also commands: open the command palette
(`Cmd+Shift+P`) and type the name. Commands that have no default shortcut are
listed under [Commands without a shortcut](#commands-without-a-shortcut), and
you can give any of them one.

## Tabs and panes

| Action | Shortcut |
| --- | --- |
| New tab | `Cmd+T` |
| New tab in Home | `Cmd+N` |
| Close pane | `Cmd+W` |
| Close tab | `Cmd+Shift+W` |
| Split right | `Cmd+D` |
| Split down | `Cmd+Shift+D` |
| Focus pane left / right / up / down | `Cmd+Opt+Arrows` |
| Zoom pane | `Cmd+Shift+Return` |
| Equalize splits | `Cmd+Ctrl+=` |
| Resize pane left / right / up / down | `Cmd+Ctrl+Shift+Arrows` |
| Next / previous tab | `Cmd+]` / `Cmd+[`, or `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Move tab left / right | `Cmd+Opt+Shift+Left` / `Cmd+Opt+Shift+Right` |
| Rename tab | `Cmd+Shift+R` |
| Go to tab 1–9 | `Cmd+1`–`Cmd+9` |
| Existing terminals | `Cmd+Opt+T` |
| Show / hide floating terminals | `Cmd+Opt+F` |
| Float pane | `Cmd+Opt+Shift+F` |
| Dock floating terminal | `Cmd+Opt+Shift+D` |
| Show floating terminal in all projects / only in its project | `Cmd+Opt+Shift+A` / `Cmd+Opt+Shift+O` |
| Move tab to main window | `Cmd+Opt+Shift+M` |

## Projects and navigation

| Action | Shortcut |
| --- | --- |
| Add project | `Cmd+O` |
| Switch project | `Cmd+Opt+P` |
| Apply layout | `Cmd+Opt+L` |
| Open project in default app | `Cmd+Opt+O` |
| New worktree | `Cmd+Opt+N` |
| Switch worktree | `Cmd+Opt+W` |
| Next / previous project | `Ctrl+]` / `Ctrl+[` |
| Go to project 1–9 | `Ctrl+1`–`Ctrl+9` |
| Back / forward | `Cmd+Ctrl+Left` / `Cmd+Ctrl+Right` |
| Toggle sidebar | `Cmd+B` |
| Agents panel | `Cmd+Opt+A` |
| Agent sessions | `Cmd+Opt+S` |
| Command palette | `Cmd+Shift+P` |
| Theme picker | `Cmd+Shift+K` |
| Settings | `Cmd+,` |
| Full screen | `Cmd+Ctrl+F` |

## Terminal

| Action | Shortcut |
| --- | --- |
| Copy / paste | `Cmd+C` / `Cmd+V` |
| Find / next / previous | `Cmd+F` / `Cmd+G` / `Cmd+Shift+G` |
| Previous / next prompt | `Cmd+Up` / `Cmd+Down` |
| Font size bigger / smaller / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` |
| Clear screen | `Cmd+K` |
| Scroll to top / bottom | `Cmd+Home` / `Cmd+End` |
| Delete line | `Cmd+Backspace` |
| Start / end of line | `Cmd+Left` / `Cmd+Right` |
| Line feed (`Ctrl+J`) | `Shift+Return` |

A line feed is a new line in many AI tools, but shells treat it like Return.

## Browser

| Action | Shortcut |
| --- | --- |
| New browser tab | `Cmd+Opt+B` |
| Address bar | `Cmd+L` |
| Reload | `Cmd+R` |
| Back / forward | `Cmd+[` / `Cmd+]` |
| Find on page | `Cmd+F` |
| Zoom in / out / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` |
| Copy address | `Cmd+Shift+C` |
| Web Inspector | `Cmd+Opt+I` |

In a browser, `Cmd+[` and `Cmd+]` go back and forward instead of switching
tabs.

## Composer

| Action | Shortcut |
| --- | --- |
| Toggle composer | `Cmd+I` |
| Send / send without Return | `Cmd+Return` / `Cmd+Shift+Return` |

## Commands without a shortcut

These commands are in the command palette and in **Settings → Keyboard**, where
you can give them a shortcut.

| Area | Commands |
| --- | --- |
| Tabs | Reset tab title, toggle tab pin, close other tabs, close tabs to the left / right, new tab to the left / right, move tab to new window |
| Panes | Focus next / previous pane, move pane to new tab, move pane to new window, detach terminal |
| Projects | Rename project, remove project, copy project path, reveal project in Finder, toggle project focus, toggle worktrees, switch workspace, new workspace, servers |
| Terminal | Clear screen, select all, scroll to top, reset font size, select command output |
| Browser | Open page in default browser |
| App | Keyboard shortcuts, reload configuration, toggle status bar, create extension, manage extensions, check for updates, show welcome tour, show Muxy tip |

Clear screen, select all, scroll to top, and reset font size already have
terminal keys (see [Terminal](#terminal)). A shortcut you give them in
**Settings → Keyboard** works as well.

## Changing shortcuts

- In **Settings → Keyboard**, click a shortcut and press the new keys.
  **Reset** restores the default. Muxy refuses keys that are already taken.
- Changes are saved to `settings.toml` under `[keymap]`, for example
  `split_right = "cmd-alt-d"`.
- Terminal keys such as clear screen, line editing, and scrolling have built-in
  bindings. Add your own in **Settings → Terminal → Key bindings**; they take
  precedence while a terminal is focused.
- Commands from **Settings → Commands** and extensions can have shortcuts too.
- The Quick Terminal shortcut is set in **Settings → Quick Terminal**.
