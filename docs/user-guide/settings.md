# Settings

Open settings with `Cmd+,` (**Muxy -> Settings...**). Use search at the top to find any setting by name.

## General

- **Update channel** — *Stable* (tagged releases) or *Beta* (auto‑built per commit). Switching channels updates Sparkle's appcast immediately.
- **Auto‑expand worktrees on project switch** — automatically opens the worktree list when you switch to a project that has more than one.
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
- **Sidebar style** — controls collapsed and expanded sidebar layout.
- **Source Control display mode** — tab, attached panel, or separate window.

See [Themes](../features/themes.md).

## Rich Input

- **Rich Input** — image submission mode, position, floating mode, font, and line height.

## Keyboard Shortcuts

- All actions remappable via a key‑capture recorder.

See [Keyboard Shortcuts](keyboard-shortcuts.md).

## Commands

- Define reusable shell command shortcuts that open a new terminal tab.

See [Keyboard Shortcuts](keyboard-shortcuts.md#commands).

## Recording

- **Press Return after inserting** — sends dictated text immediately.
- **Language** — on-device speech recognition language.

See [Voice Recording](../features/voice-recording.md).

## Notifications

- **Toast** — show an in-app toast on arrival.
- **Desktop notifications** — show a macOS notification when Muxy is not frontmost.
- **Toast position** — top or bottom of the window.
- **Sound** — choose the notification sound.
- **AI Providers** — enable or disable each provider hook integration.
- **Per‑source delivery** — separate toggles for Claude Code, OpenCode, OSC sequences, and the socket API.

See [Notifications](../features/notifications.md).

## Mobile

- **Allow Mobile Connections** — start / stop the WebSocket server.
- **Port** — defaults to 4865.
- **Pair Mobile Device** — shows the pairing QR code.
- **Approved devices** — list of paired clients with revoke buttons.

See [Remote Server](../remote-server/overview.md).

## Remote Devices

- **Remote devices** — reusable SSH connections for remote workspaces.
- **Environment** — `KEY=value` variables exported before remote terminals, git, files, worktrees, and extension commands run. New SSH devices default to `TERM=xterm-256color`.

## AI Assistant

- **AI Assistant Tool** — CLI used for commit and PR generation.
- **Model overrides** — optional Claude, Codex, or OpenCode model names.
- **Custom AI Command** — command used when the custom provider is selected.
- **Commit Prompt** — prompt used to generate commit messages.
- **Pull Request Prompt** — prompt used to generate PR drafts.

## JSON

The JSON tab exposes editable settings as `settings.json`.

Use it for bulk edits, sharing settings, or editing values faster than clicking through controls. Muxy validates the file before applying it.
