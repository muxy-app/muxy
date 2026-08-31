# Settings

Open settings with `Cmd+,` (**Muxy -> Settings...**). Use search at the top to find settings by name. Search
matches the active app language as well as English setting keys and technical aliases.

## Composer

Open **Settings → Composer** to configure image submission, editor typography, and automatic draft clearing. Composer itself is panel-only. Use its header controls to move it between the right and bottom, pin or float it inside the main window, resize it, toggle broadcast, and change its font size. Those panel values persist in the selected profile's `preferences.json`.

**Image Submission** chooses how copied images reach local terminal panes. **Clipboard Paste** temporarily replaces the native pasteboard, sends `Ctrl+V`, waits 300 ms, and restores the full captured pasteboard. **Inline File Path** inserts the shell-escaped path of the normalized private PNG. Real TUI image support is application-dependent.

**Font Family** mirrors `richInputFontFamily` from `editor-settings.json`. **Line Height** stores `richInputLineHeightMultiplier`, clamped from 1.1× through 2.0×. The panel header's font controls persist `muxy.richInput.fontSize` separately.

**Clear After Sending** clears text and attachments only when every target succeeds and the draft has not changed while submission was running. **Clear on Close** clears the released worktree draft when Composer closes or transfers to another worktree; a same-worktree pane change preserves it. Both options are off by default and are stored as `muxy.richInput.clearAfterSending` and `muxy.richInput.clearOnClose` in `settings.json`.

Drafts are private per-worktree data in `rich-input-drafts.json`. Copied images are private files under `RichInputImages/`; startup removes only files proven to have no durable draft reference. No Composer state is imported through the legacy Swift migration allowlist.

## Language

English is built in. Enabled extensions can provide additional app languages, and every provider appears under
**Interface → Language** with the extension name so you can choose between multiple providers for the same language.
Choose **Browse Language Extensions…** to open the Extension Store already filtered to available language packs.
After installing and enabling a pack, its languages appear automatically in the app-language picker.
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
global setting. CLI creation without `--path` resolves a project template first, then a project folder, then the global
template, then the global folder. The native **App Default** choice intentionally starts at the global template and
global folder; choosing **Template** or **Folder** in the dialog stores a project override for later CLI creation. If no
applicable setting exists, Muxy uses the profile-local `worktree-checkouts/<project-id>/<worktree-name>` directory.
Explicit CLI paths always win. Relative and `~` paths are resolved before Git runs, and remote worktrees keep their
remote workspace layout.

## Worktree lifecycle hooks

Muxy can run setup and teardown commands for Muxy-managed local worktrees. Put project-specific hooks in
`<project>/.muxy/worktree.json`. The per-machine file is `$XDG_CONFIG_HOME/muxy/worktree.json` when
`XDG_CONFIG_HOME` is set to a non-empty value, or `~/.config/muxy/worktree.json` otherwise.

Both files use the same format. Commands may be strings or objects with a `command` and optional `name`:

```json
{
  "setup": [
    "docker compose up -d",
    { "name": "Install dependencies", "command": "pnpm install" }
  ],
  "teardown": [
    "docker compose down"
  ]
}
```

The creation dialog labels each setup command as **Per-machine** or **Project**. Project hook files may come from the
repository, so review those commands before enabling them. Per-machine commands come from your local configuration.

Setup commands run after Muxy creates and registers a managed local worktree. Per-machine setup runs before project
setup. The creation dialog lists every command and keeps setup disabled until you explicitly enable it for that
worktree. Approval covers the displayed project commands only; if the project configuration changes before execution,
setup stops. CLI, mobile, and API creation do not run setup hooks because they have no per-run command confirmation.
Each command runs in its own shell with a shared five-minute total budget. A failed setup command stops later setup
commands and is logged, but does not undo the successfully created worktree.

Teardown commands run before Git removes the worktree, in the reverse layer order: project teardown first, then
per-machine teardown. Native worktree removal lists every teardown command in its confirmation. Confirming approves
only the displayed project commands; removal stops if those commands change before execution. Removals without that
native confirmation, including mobile, API, extension, and bulk project cleanup paths, skip project teardown and run
only the pre-authorized per-machine commands. A teardown command failure or invalid configuration stops the removal and
leaves the worktree registered. Every hook command uses the worktree as its working directory and receives
`MUXY_PROJECT_PATH`, `MUXY_WORKTREE_ID`, `MUXY_WORKTREE_PATH`, `MUXY_WORKTREE_NAME`, and `MUXY_WORKTREE_BRANCH`. Hooks do
not run for remote or externally managed worktrees.

Creation and removal share one monotonic five-minute budget across every hook and Git subprocess. A failure before Git
creates or removes the checkout leaves tracking, workspace, panes, preferences, history, and selection unchanged. Once
Git creation succeeds, later setup, tracking, workspace, preference, or reconciliation failures are warnings and never
delete the checkout. Once Git removal succeeds, or reconciliation proves a failed/timed-out Git command already removed
it, Muxy completes exact worktree-ID cleanup in memory; persistence failures are warnings and never claim the deletion
was rolled back. If the primary repository directory is missing during removal, Muxy removes the stale app identity but
preserves the secondary checkout files and reports that explicitly.

Git and hook execution run in headless services rather than views. On Unix, timed-out commands terminate their process
group, drain output, kill descendants after a short grace period, and reap the direct child. The hook environment adds
the five `MUXY_*` worktree variables above while retaining the normal process environment and configured shell.

## Project sorting and navigation

The project sort menu supports Manual, Name (A–Z), Name (Z–A), Recently Active, and Date Added. Pinned projects remain
first in every mode. The selected mode is applied immediately and stored as `muxy.projectSortMode` in `settings.json`.

Back and Forward navigate complete project, worktree, area, and optional tab entries. Muxy keeps up to 100 entries,
skips entries whose target no longer exists, and prunes deleted projects and worktrees. A failed workspace save does not
move the history cursor or leave a partially applied selection.

## Focused-layout worktree grouping

In **Appearance → Sidebar**, select **Tab Focused** or **Agents Focused** to show **Nest worktrees inside projects**.
It is off by default. Turn it on to nest all worktrees under their project; turn it off to keep worktrees as top-level rows. Tab
Focused shows top-level worktrees only when they have open tabs, while Agents Focused shows every secondary worktree.

## Top bar actions

Turn off **Settings → Interface → Interface → Show Top Bar Actions** to hide every window-level control on the right
side of the top bar. The top bar remains visible in every layout, including the tab strip and its new-tab button in
Projects Focused and Agents Focused. Pane-local tab strips and their controls are unaffected. The preference is stored
as `muxy.showTopBarActions` in `settings.json` and defaults to on.

## Extension icon rail

Turn on **Settings → Interface → Interface → Show Extension Icon Rail** to show visible `togglePanel` extension
topbar icons in a right-hand rail. The title bar spans the remaining window width after the left sidebar; the rail
starts below the title-bar hairline and runs to the window bottom beside the status bar. Popover and other command
icons stay in the title bar. Off by default. The rail hides when no panel-toggle items are visible. The rail is
independent of **Show Top Bar Actions** (`muxy.showTopBarActions`). The preference is stored as
`muxy.showExtensionIconRail` in `settings.json`. Rail icon order is stored as `muxy.extensionIconRailOrder` and
contains rail IDs only.

## Project search

In the Project Focused layout, turn on **Settings → Interface → Sidebar → Always Show Project Search** to keep the
project search field visible whenever the sidebar uses its expanded style. The setting is off by default, and the
search field is unavailable while it is off or while the sidebar uses an icon-only style. The preference is stored as
`muxy.showProjectSearch` in `settings.json`.

## Sidebar tips

Muxy shows one tip at the bottom of the built-in sidebar. The starting tip is selected when Muxy launches and stays
stable until you use the previous or next button. In an icon-only sidebar, select the lightbulb button to open the same
tip in a popover.

Select the close button on a tip, then confirm **Hide Tips** to hide tips. Turn on
**Settings → Interface → Sidebar → Show Tips** to show them again. The preference is stored as `muxy.tips.visible` in
`settings.json`. Extension-provided sidebars control their own content and do not show the built-in tip card.

## Background sessions

Open **Settings → Terminal → Background sessions**. **Run new terminals in the background** is off by default and is stored as `muxy.terminalPersistentSession.enabled` in `settings.json`.

This setting is restart-only. Enabling it does not adopt or replace terminals in the current process. After a normal Quit and fresh launch, Muxy creates or recovers daemon-backed sessions for eligible local workspace terminal tabs before creating renderers. Only visible panes receive a renderer; hidden tabs stay backed by their daemon shell without a Ghostty surface.

Disabling asks for confirmation with the number of sessions that will end. The current process keeps its applied mode. On the next fresh launch, Muxy ends every tracked session process tree before clearing saved session IDs and starting ordinary terminals. Remote terminals and Quick Terminal are always excluded.

Closing a tab ends its persistent session. Use **Send to Background** when you want to remove a tab but keep its processes. Ordinary Quit only detaches renderers. The **Terminal Sessions** status item lists Workspace Sessions, Background Sessions, and Missing/Ended links, with state-dependent Focus, Reattach, End, Start New, and Remove actions. Its footer provides End All Sessions and Terminal Settings.

Each session has one renderer at a time and replays at most 256 KiB of recent ordinary output when reattached. Older output can be omitted, and alternate-screen output is not retained as ordinary replay. zsh, bash, fish, elvish, and nushell receive the bundled shell integrations.

Use `muxy list-sessions` and `muxy kill-session --session <id>` while Muxy is open. See [Background sessions](../features/background-sessions.md) for lifecycle, recovery, security, cleanup, and acceptance details.

## Idle terminal memory

Open **Settings → Terminal → Memory**. **Free idle inactive terminals** is off by default and is stored as `muxy.terminalOffline.enabled`. It applies only to hidden terminals. Visible panes, alternate-screen programs, active foreground or background descendants, pending input or resize work, and uncertain process state stay awake.

The timeout is stored as `muxy.terminalOffline.idleThresholdSeconds` and defaults to 300 seconds. Available choices are 10 seconds, 30 seconds, 1 minute, 2 minutes, 5 minutes, 10 minutes, 15 minutes, and 30 minutes.

A persistent terminal sleeps by releasing only its renderer and attach client. Its daemon shell continues, and wake reattaches with bounded replay. A safe ordinary terminal releases its shell and Ghostty surface. Wake restores its last working directory without rerunning a one-shot startup command, but ordinary scrollback is lost.

## Resource usage

Open **Settings → Appearance → Interface**. **Show Resource Usage in Status Bar** is on by default and is stored as `muxy.showResourceUsageInStatusBar`.

The status item samples Muxy's app process tree plus authenticated session daemon, shell, and descendant trees. It reports CPU and resident memory, deduplicates overlapping identities, and distinguishes live, stale, and unavailable data. Turning it off stops polling immediately and clears the previous CPU baseline and displayed sample. Composer and Terminal Sessions remain available in the same trailing status group.

## Quick terminal

The assigned shortcut is the only way to open the quick terminal. Trigger it again, use the panel's close button, or click outside the panel to hide it while retaining its shell. Use `Cmd+W` to close the panel and release the surface; the next trigger starts a fresh shell. The panel opens centered near the top of the pointer's current display with a small gap below the usable screen edge. Open **Quick Terminal** in Settings to configure its shortcut, size, and appearance:

- **Enable Quick Terminal** controls the entire feature. Turning it off stops the shortcut listener, closes the panel, and releases its shell while preserving its settings.
- No shortcut is assigned by default.
- **Double Shift** requires macOS Input Monitoring for use outside Muxy.
- **Option Space** or another recorded key combination is registered as a conventional global shortcut without Input Monitoring.
- **Width** and **Height** set the panel size in points. Saved changes apply the next time Quick Terminal opens, and smaller displays automatically reduce the configured size.
- **Terminal transparency** controls how much of the desktop shows through the terminal background tint from 0–55% on the next opening.
- **Background vibrancy** stores a value from 0–100%. A value of 0 disables window blur; any nonzero value enables the same native window-level blur on the next opening.

The vibrancy value is not a custom blur radius or a continuous material-strength control. Before each opening, the hidden panel and terminal surface are prepared at the final display-constrained size. Show and hide then use a clipping reveal instead of resizing the terminal viewport during the animation. The bridge and controls use the active terminal theme. Quick Terminal follows the separately configured light and dark terminal themes, applies changes to its retained surface, and switches live when macOS appearance changes.

The gear button hides Quick Terminal and opens the main app Settings directly on the Quick Terminal category. Size and appearance changes are persisted without modifying the currently visible panel and are loaded on its next opening. The shortcut is also available from the shortcut control in the quick terminal. The feature toggle is stored as `muxy.quickTerminal.enabled` in `settings.json`. The shortcut is stored as `shortcuts.quickTerminal` using `{"type":"unassigned"}`, `{"type":"doubleShift"}`, or `{"type":"keyCombo","keyCombo":{"key":"space","modifiers":...},"virtualKeyCode":49}`. Panel dimensions are stored as `muxy.quickTerminal.width` and `muxy.quickTerminal.height`. Appearance settings use `muxy.quickTerminal.transparency` as an integer percentage from 0–55 and `muxy.quickTerminal.blur` as a stored integer from 0–100 with the binary mapping described above.

The shortcut status reports disabled, unassigned, local-only, system-wide, unavailable, or a registration error based on the active runtime. Muxy requests Input Monitoring only when you choose **Enable Input Monitoring**. Denial or revocation leaves Double Shift available while Muxy is active, and a conventional global shortcut remains available without Input Monitoring.

When macOS Reduce Transparency or Increase Contrast is enabled, Muxy temporarily renders the quick terminal as opaque and unblurred without changing the saved appearance settings. Quick Terminal is a macOS-only runtime. Its portable settings remain readable on Linux, but the settings category and panel are not exposed there.

## App transparency

Open **Settings → Interface → Appearance** to make the main window's workspace transparent:

- **App transparency** controls how much of the desktop shows through terminal panes, the top bar, and the status bar from 0–55%. The default is 0, which keeps the window opaque.
- **App vibrancy** continuously controls the native macOS material intensity from 0–100%. The main window itself stays opaque, so the desktop shows through the vibrancy material and the effect needs a vibrancy above zero.

The sidebar keeps its separate **Vibrancy** toggle in the same settings category. Both sliders apply immediately to open terminals, including split panes, and to the top bar and status bar. The values are stored in `settings.json` as `muxy.app.transparency` (integer percentage 0–55) and `muxy.app.blur` (integer material intensity 0–100).

When macOS Reduce Transparency or Increase Contrast is enabled, Muxy renders the workspace opaque and unblurred without changing the saved settings. A terminal pane controlled by a remote device keeps its opaque client theme until control returns to the Mac.
