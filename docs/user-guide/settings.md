# Settings

Open Settings with `Cmd+,`. Use the search box to find a setting. Changes apply
immediately, and Settings works while the server is offline.

## Sections

| Section | What's in it |
| --- | --- |
| General | Closing behavior, the app that opens files, window size, worktree locations |
| Quick Terminal | Global shortcut, size, transparency |
| Composer | Dictation, clearing, image submission, font |
| Appearance | Language, light and dark themes, sidebar, status bar, tips |
| Keyboard | Every shortcut. See [Keyboard shortcuts](keyboard-shortcuts.md) |
| Commands | Your own commands, each run in a new tab, with optional shortcuts |
| Terminal | Font, copy on select, new pane folder, `ghostty.conf` |
| Server | This computer's [server](../features/server.md#settings): shell, history, shell integration, stop and restart |
| Mobile | [Phone access](../features/mobile.md) and paired phones |
| Extensions | Installed extensions and the marketplace |
| AI | Tools and prompts for [Git](../features/git.md) actions |
| Backup & Restore | Export, restore, and import from 1.x |

## Files

Settings are plain files in Muxy's
[profile folder](../features/server.md#files). **Edit in…** at the top of a
section opens its file.

| File | Holds |
| --- | --- |
| `settings.toml` | App preferences, shortcuts (`[keymap]`), commands, remote servers |
| `ghostty.conf` | Terminal font, colors, and key bindings |
| `themes/` | Your own themes |
| `server.toml` | Server settings |

- Restart the app after editing `settings.toml` by hand. If the file has an
  error or an unknown key, the app won't open; see
  [Troubleshooting](troubleshooting.md#the-app-doesnt-open).
- `ghostty.conf` reloads with **Reload Configuration** in the app menu, and
  whenever the Muxy window becomes active.

## Terminal configuration

`ghostty.conf` uses [Ghostty's format](https://ghostty.org/docs/config). Muxy
reads these options and lists any others under **Settings → Terminal →
Configuration warnings**:

- **Fonts:** `font-family` (and bold, italic variants), `font-size`,
  `font-feature`, `font-codepoint-map`, `font-thicken`, `adjust-cell-height`,
  `adjust-cell-width`
- **Colors:** `background`, `foreground`, `palette`, `cursor-*`,
  `selection-*`, `bold-is-bright`, `background-opacity`
- **Window:** `window-padding-x`, `window-padding-y`, `window-padding-balance`,
  `window-padding-color`
- **Input and mouse:** `keybind`, `macos-option-as-alt`, `copy-on-select`,
  `mouse-reporting`, `mouse-scroll-multiplier`, `scroll-to-bottom`
- **Includes:** `config-file`

## Themes

Muxy ships about 490 themes. Pick the app's light and dark themes in
**Settings → Appearance**, or press `Cmd+Shift+K`. Muxy follows the macOS
appearance.

Custom themes in the profile's `themes/` folder show up too. The chosen theme
colors the app and its terminals.

## Backup and restore

- **Export…** saves a `.muxy` file with your settings, `ghostty.conf`, themes,
  server settings, extension preferences, remote server list, and local
  projects with their layouts.
- **Choose file…** under **Restore backup** restores on the next launch. Muxy
  keeps a recovery copy in the profile's `Backups/` folder. Existing projects
  are kept and get the saved layouts.
- Backups never include running terminals, paired phones, extension packages,
  remote projects, or project files.

## Import from Muxy 1.x

**Import installed 1.x** reads Muxy 1.x's settings from this Mac. **Choose
file…** takes a 1.x `.muxy` backup or `settings.json`. Muxy imports supported
settings, shortcuts, custom commands, local projects, workspaces, worktrees, and
terminal tabs, and lists what it skipped.

## Extensions

**Settings → Extensions → Browse** installs extensions from the marketplace.
New extensions start disabled; review their permissions and turn them on.
Updates are manual: use **Update all**, or update one from its page. Building
your own? See [Extensions](../extensions/overview.md).

Language packs are extensions too. Pick a language in **Settings → Appearance →
App language**, or choose **Browse language extensions…**.
