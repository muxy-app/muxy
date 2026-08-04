# Settings

Open settings with `Cmd+,` (**Muxy -> Settings...**). Use search at the top to find settings by name. Search
matches the active app language as well as English setting keys and technical aliases.

## Language

English is built in. Enabled extensions can provide additional app languages, and every provider appears under
**Interface → Language** with the extension name so you can choose between multiple providers for the same language.
If the selected extension is disabled, removed, or temporarily invalid, Muxy keeps the selection and uses English
until that provider becomes available again.

Translation providers contain resource-only catalogs and cannot add executable code through the language feature.
Extension authors can follow the [localization provider guide](../extensions/localizations.md).

## Updates

Muxy checks for updates automatically and downloads available releases in the background. Sparkle can offer
**Install on Quit** for a downloaded release, applying it the next time Muxy quits without interrupting current work.
Choose **Install and Relaunch** to apply the update immediately when that option is presented.

Use **Install downloaded updates on quit** to control this behavior. Muxy saves workspace and draft state before the
terminal shutdown cleanup begins, so a normal update-driven restart restores the last saved workspace.
The same setting is available as `SUAutomaticallyUpdate` in `settings.json`.

## Worktree path templates

Set the default under **Projects -> Worktrees** and choose **Template**. Every template must include `{branch}` and can
also use these filesystem-safe values:

- `{project-name}` — the project name shown in Muxy
- `{base-dir}` — the current checkout folder name
- `{branch}` — the branch name, with path separators replaced

Relative templates start from the project folder. For a project at `/code/my-app` and branch `feature/auth`,
`../{base-dir}.{branch}` resolves to `/code/my-app.feature-auth`.

Choose **Folder** to retain Muxy's existing folder layout. A global folder stores worktrees under
`<folder>/<project-name>/<worktree-name>`, while a folder selected in the new worktree dialog stores them under
`<folder>/<worktree-name>`. A project-specific template or folder selected in that dialog takes precedence over the
global setting. Remote worktrees keep their remote workspace layout.

## Focused-layout worktree grouping

In **Appearance → Sidebar**, select **Tab Focused** or **Agents Focused** to show **Nest worktrees inside projects**.
It is off by default. Turn it on to nest all worktrees under their project; turn it off to keep worktrees as top-level rows. Tab
Focused shows top-level worktrees only when they have open tabs, while Agents Focused shows every secondary worktree.

## Sidebar tips

Muxy shows one tip at the bottom of the built-in sidebar. The starting tip is selected when Muxy launches and stays
stable until you use the previous or next button. In an icon-only sidebar, select the lightbulb button to open the same
tip in a popover.

Select the close button on a tip, then confirm **Hide Tips** to hide tips. Turn on
**Settings → Interface → Sidebar → Show Tips** to show them again. The preference is stored as `muxy.tips.visible` in
`settings.json`. Extension-provided sidebars control their own content and do not show the built-in tip card.

## Background sessions

Open **Settings → Terminal → Background sessions** to keep terminals running after Muxy quits:

- **Run new terminals in the background** starts each new terminal in a separate background process, like tmux. Quitting Muxy leaves those terminals running, and reopening it reconnects them along with their recent output.
- Only terminals opened after the setting is switched on are affected. Terminals that are already open keep their current behavior.
- Reopening Muxy reattaches every restored tab whose session is still running, without waiting for you to click the tab.
- Closing a tab ends its session. Right-click an eligible local terminal and choose **Send to Background** to close its tab without stopping its processes; the session then stays available from the status bar.
- If Muxy loses its connection to a still-running session, it reconnects on its own. A tab only closes when the session itself has ended; when reconnecting keeps failing the tab waits with a **Reconnect** button instead, and the session keeps running.
- Turning the setting off asks for confirmation and then stops every terminal still running in the background.
- Remote SSH terminals and the quick terminal are never run this way.

The status bar shows how many background terminals in the current project and worktree are **not** open in a tab. Its popover lists those and can point the focused tab at one, open a new tab attached to one, or stop one. It disappears when nothing is waiting, so it only appears when you have something to recover.

Background terminals keep working-directory tracking, tab titles, and AI progress in zsh, bash, fish, elvish, and nushell. Other shells run normally but lose those integrations, the same limitation tmux has.

The setting is stored as `muxy.terminalPersistentSession.enabled` in `settings.json`. Use `muxy list-sessions` and `muxy kill-session --session <id>` to inspect and stop sessions from a shell.

## Quick terminal

The assigned shortcut is the only way to open the quick terminal. On a display with a camera cutout, the terminal expands out of it like a dynamic island. Open **Quick Terminal** in Settings to configure its shortcut, size, and appearance:

- **Enable Quick Terminal** controls the entire feature. Turning it off stops the shortcut listener, closes the panel, and releases its shell while preserving its settings.
- No shortcut is assigned by default.
- **Double Shift** requires macOS Input Monitoring for use outside Muxy.
- **Option Space** or another recorded key combination is registered as a conventional global shortcut without Input Monitoring.
- **Width** and **Height** set the panel size in points for the next opening. Smaller displays automatically reduce the configured size.
- **Terminal transparency** controls how much of the desktop shows through the terminal workspace from 0–55%.
- **Background vibrancy** continuously controls the native macOS material intensity from 0–100%. The cutout bridge remains solid.

The vibrancy control mixes the system material continuously; it does not set a custom blur radius.

The gear button in the quick terminal opens an in-place settings popover with the transparency, vibrancy, width, and height controls, so those can be adjusted without leaving the terminal. Transparency and vibrancy apply immediately; size applies when the slider is released. The shortcut is also available from the shortcut control in the quick terminal. The feature toggle is stored as `muxy.quickTerminal.enabled` in `settings.json`. The shortcut is stored as `shortcuts.quickTerminal` using `{"type":"unassigned"}`, `{"type":"doubleShift"}`, or `{"type":"keyCombo","keyCombo":{"key":"space","modifiers":...},"virtualKeyCode":49}`. Panel dimensions are stored as `muxy.quickTerminal.width` and `muxy.quickTerminal.height`. Glass settings use `muxy.quickTerminal.transparency` as an integer percentage from 0–55 and `muxy.quickTerminal.blur` as an integer material intensity from 0–100.

When macOS Reduce Transparency or Increase Contrast is enabled, Muxy temporarily renders the quick terminal as opaque and unblurred without changing the saved glass settings.

## App transparency

Open **Settings → Interface → Appearance** to make the main window's workspace transparent:

- **App transparency** controls how much of the desktop shows through terminal panes, the top bar, and the status bar from 0–55%. The default is 0, which keeps the window opaque.
- **App vibrancy** continuously controls the native macOS material intensity from 0–100%. The main window itself stays opaque, so the desktop shows through the vibrancy material and the effect needs a vibrancy above zero.

The sidebar keeps its separate **Vibrancy** toggle in the same settings category. Both sliders apply immediately to open terminals, including split panes, and to the top bar and status bar. The values are stored in `settings.json` as `muxy.app.transparency` (integer percentage 0–55) and `muxy.app.blur` (integer material intensity 0–100).

When macOS Reduce Transparency or Increase Contrast is enabled, Muxy renders the workspace opaque and unblurred without changing the saved settings. A terminal pane controlled by a remote device keeps its opaque client theme until control returns to the Mac.
