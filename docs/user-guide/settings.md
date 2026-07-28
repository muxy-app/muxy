# Settings

Open settings with `Cmd+,` (**Muxy -> Settings...**). Use search at the top to find settings by name.

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
