# Keyboard Shortcuts

Defaults for the desktop app. Change any of them in **Settings → Keyboard**.
For the terminal UI, see [Terminal UI keys](../features/terminal-ui.md#keys).

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
| Next / previous tab | `Cmd+]` / `Cmd+[`, or `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Go to tab 1–9 | `Cmd+1`–`Cmd+9` |
| Existing terminals | `Cmd+Opt+T` |

## Projects and navigation

| Action | Shortcut |
| --- | --- |
| Add project | `Cmd+O` |
| Next / previous project | `Ctrl+]` / `Ctrl+[` |
| Go to project 1–9 | `Ctrl+1`–`Ctrl+9` |
| Back / forward | `Cmd+Ctrl+Left` / `Cmd+Ctrl+Right` |
| Toggle sidebar | `Cmd+B` |
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

## Composer and dictation

| Action | Shortcut |
| --- | --- |
| Toggle composer | `Cmd+I` |
| Send / send without Return | `Cmd+Return` / `Cmd+Shift+Return` |
| Dictation | `Cmd+Shift+I` |

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
