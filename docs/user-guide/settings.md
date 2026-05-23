# Settings

Open settings with `Cmd+,` (**Muxy -> Settings...**). Use search at the top to find any setting by name.

## General

- **Update channel** — *Stable* (tagged releases) or *Beta* (auto‑built per commit). Switching channels updates Sparkle's appcast immediately.
- **Auto‑expand worktrees on project switch** — automatically opens the worktree list when you switch to a project that has more than one.
- **File tree root directory** — follow the project root or the active terminal directory.
- **Project picker** — use Muxy's picker or the Finder picker.
- **Project picker default path** — default folder for Muxy's picker.
- **Default worktree path** — parent folder for new worktrees.
- **Auto-copy terminal selection** — copies terminal selections when the mouse is released.
- **Keep projects open after closing all tabs** — keeps a project visible in the sidebar even after its last tab is closed.
- **Confirm before closing tab with running process** — prompts before killing a non‑idle terminal.
- **Confirm before quitting Muxy** — confirmation dialog on `Cmd+Q`. Includes a "Don't ask again" toggle.
- **Crash reports** — controls anonymous crash report consent when diagnostics are available.

## Appearance

- **Interface size** — changes app density.
- **Show status bar** — shows or hides the bottom status bar.
- **Theme** — paired light / dark terminal theme picker.
- **Syntax highlighting theme** — applied to the built‑in editor.
- **Sidebar style** — controls collapsed and expanded sidebar layout.
- **Source Control display mode** — tab, attached panel, or separate window.

See [Themes](../features/themes.md).

## Editor

- **Default editor** — built‑in Muxy editor, or an external command.
- **External editor command** — used when default is set to "external". `{file}`, `{line}`, `{column}` placeholders are substituted. Terminal Command runs through your login interactive shell.
- **Markdown preview** — remote images, font, and zoom.
- **HTML default view** — default mode for HTML files.
- **Rich Input** — image submission mode, position, floating mode, font, and line height.
- **Appearance** — current-line highlight, line numbers, wrapping, font, size, and line height.

## Sessions

- **Restore Terminal Sessions** — recreates terminal tabs after restart.
- **Blocked Commands** — commands Muxy will not restore automatically.

See [Session Restore](../features/session-restore.md).

## Keyboard Shortcuts

- All actions remappable via a key‑capture recorder.
- **Custom Commands** — define reusable shell command shortcuts.

See [Keyboard Shortcuts](keyboard-shortcuts.md).

## Recording

- **Press Return after inserting** — sends dictated text immediately.
- **Language** — on-device speech recognition language.

See [Voice Recording](../features/voice-recording.md).

## Notifications

- **Toast position** — top or bottom of the window.
- **Sound** — choose the notification sound.
- **Per‑source delivery** — separate toggles for Claude Code, OpenCode, OSC sequences, and the socket API.

See [Notifications](../features/notifications.md).

## Mobile

- **Allow Mobile Connections** — start / stop the WebSocket server.
- **Port** — defaults to 4865.
- **Pair Mobile Device** — shows the pairing QR code.
- **Approved devices** — list of paired clients with revoke buttons.

See [Remote Server](../remote-server/overview.md).

## AI Assistant

- **AI Assistant Tool** — CLI used for commit and PR generation.
- **Model overrides** — optional Claude, Codex, or OpenCode model names.
- **Custom AI Command** — command used when the custom provider is selected.
- **Commit Prompt** — prompt used to generate commit messages.
- **Pull Request Prompt** — prompt used to generate PR drafts.

## AI Usage

- **Enable AI usage tracking** — global toggle.
- **Display mode** — show *used* or *remaining* values.
- **Auto‑refresh** — Off / 5m / 15m / 30m / 1h.
- **Show secondary limits** — keep / hide non‑primary metrics.
- **Per‑provider toggles** — enable each provider individually.

See [AI Usage](../features/ai-usage.md).

## JSON

The JSON tab exposes editable settings as `settings.json`.

Use it for bulk edits, sharing settings, or editing values faster than clicking through controls. Muxy validates the file before applying it.
