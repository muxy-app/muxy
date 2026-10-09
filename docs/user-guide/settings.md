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
| Terminal | Fonts, spacing, transparency, vibrancy, colors, cursor, padding, selection, scrolling, Option keys, key bindings |
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
| `terminal.toml` | Terminal preferences and key bindings |
| `themes/` | Your own themes |
| `server.toml` | Server settings |

- Restart the app after editing `settings.toml` by hand. If the file has an
  error or an unknown key, the app won't open; see
  [Troubleshooting](troubleshooting.md#the-app-doesnt-open).
- Hand edits to `terminal.toml` apply when Muxy or Settings becomes active. If
  the file has an error, Settings shows it and keeps the last good values.

## Terminal configuration

Use **Settings → Terminal** for:

- Font family, fallback fonts, bold and italic fonts, size, line and character
  spacing, ligatures and other font features, fonts for Unicode ranges, thicker
  strokes, and bright colors for bold text.
- Background transparency and vibrancy sliders, optionally including colored cells.
  Vibrancy uses the native macOS window material, including its blur and desktop
  tinting. Increasing vibrancy reveals more of that material by reducing the
  terminal's solid color overlay; it also works with transparency set to zero.
  Reduce Transparency and Increase Contrast disable this material.
- Cursor shape, blinking, opacity, and thickness. Thickness accepts an adjustment in
  physical pixels or a percentage: `1` adds one pixel and `100%` doubles it.
  It applies to bar, underline, and outlined cursors. Terminal programs can
  override cursor shape and blinking.
- Separate sliders for left, right, top, and bottom padding, with balanced
  unused space and a padding color.
- Background, text, cursor, selection, and the 16 ANSI colors. Each replaces
  the theme's color; leave it empty to use the theme.
- Copy on select, clearing selection when typing or copying, mouse reporting,
  separate trackpad and mouse wheel speeds, and scrolling to live output.
- Whether both, neither, left, or right Option keys act as Alt.
- The working directory for new panes.
- Key bindings the terminal handles while focused: record a key, then pick an
  action such as sending text. They win over app shortcuts. You can also turn
  off the built-in terminal bindings.

Numeric controls use sliders, on/off options use switches, and fixed choices use
dropdowns. Sliders preview changes while dragging and save when released or when
Settings closes. Spacing and thickness offer a pixels/percent dropdown; changing
units keeps the numeric adjustment within the supported range.

Themes remain in **Settings → Appearance**. Shell and history limits remain
in **Settings → Server**. Quick Terminal opacity combines with terminal opacity.

On the first launch without `terminal.toml`, Muxy imports the supported values
from its old `ghostty.conf`, including included files and custom terminal key
bindings. The original files stay intact. Later edits to those old files have
no effect. Any unsupported legacy options appear under **Imported settings**
until you dismiss them.
Older backups and 1.x imports are converted to native preferences too. Legacy blur
and glass presets become native background vibrancy.

## Themes

Muxy ships about 490 themes. Pick the app's light and dark themes in
**Settings → Appearance**, or press `Cmd+Shift+K`. Muxy follows the macOS
appearance.

Custom themes in the profile's `themes/` folder show up too. The chosen theme
colors the app and its terminals.

## Backup and restore

- **Export…** saves a `.muxy` file with your settings, `terminal.toml`, themes,
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
terminal tabs, and lists what it skipped. Projects, tabs, and workspaces you
already have are kept. **Import installed 1.x** also brings the default shell
and extensions.

## Extensions

**Settings → Extensions → Browse** installs extensions from the marketplace.
New extensions start disabled; review their permissions and turn them on.
Updates are manual: use **Update all**, or update one from its page. Building
your own? See [Extensions](../extensions/overview.md).

Language packs are extensions too. Pick a language in **Settings → Appearance →
App language**, or choose **Browse language extensions…**.
