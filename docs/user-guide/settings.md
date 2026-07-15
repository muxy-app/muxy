# Settings

Open settings with `Cmd+,` (**Muxy -> Settings...**). Use search at the top to find settings by name.

## App

- **Update Channel** — stable releases or beta builds.
- **Confirm Quit** — asks before quitting Muxy.
- **Crash Reports** — controls anonymous crash report consent when diagnostics are available.

## Projects

- **Project Picker** — use Muxy's picker or the Finder picker.
- **Project Picker Search Location** — folder recursively searched by Muxy's picker.
- **Keep Projects Open** — keeps projects in the sidebar after closing the last tab.
- **Default Opener** — chooses the IDE or extension opener for files opened from native surfaces.
- **Default Worktree Path** — parent folder for new worktrees.

## Remote Devices

- Add and manage reusable SSH connections for remote workspaces.
- Configure `KEY=value` environment variables exported before remote terminals, git, files, worktrees, and extension commands run. New SSH devices default to `TERM=xterm-256color`.

## Interface

- **Interface Size** — controls app density across Default, Large, Extra Large, and Huge. Scales fonts, spacing, icons, and the status bar.
- **Tab header width** — controls maximum tab header width.
- **Show Status Bar** — shows or hides the bottom status bar.
- **Show Resource Usage in Status Bar** — shows CPU and memory usage; disabling it stops sampling.
- **Light Terminal Theme** and **Dark Terminal Theme** — paired terminal themes that follow macOS appearance.
- **Sidebar Vibrancy** — on by default; uses theme-tinted native macOS vibrancy across the sidebar and its traffic-light/title strip. Turn it off for a solid theme background. The main topbar keeps the active theme background.
- **Auto-expand Worktrees** — reveals worktrees when switching projects.
- **Show Home** — shows the permanent Home project at the top of the sidebar.
- **Active Sidebar** — chooses the built-in sidebar or one provided by an extension.
- **Collapsed Sidebar Style** and **Expanded Sidebar Style** — controls sidebar presentation.
- **Worktree switcher options** — unread indicators and recent-use ordering.

See [Themes](../features/themes.md).

## Terminal

- **Auto-copy Terminal Selection** — copies terminal selections when the mouse is released.
- **Confirm Running Process Tab Close** — asks before closing a terminal tab with a running process.
- Terminal config is stored in `~/Library/Application Support/Muxy/ghostty.conf` and can be opened from the Muxy menu.

See [Terminal](../features/terminal.md).

## Browser

- Enable or disable the built-in browser.
- Choose whether terminal links open in the built-in browser.
- Choose search engine and home page.
- Manage browser profiles, clear profile data, and import supported browser data.

## Rich Input

- Configure image submission mode, position, floating mode, font, and line height.

See [Rich Input](../features/rich-input.md).

## Shortcuts

- Remap app actions with the key-capture recorder.

See [Keyboard Shortcuts](keyboard-shortcuts.md).

## Commands

- Define reusable shell command shortcuts that open a new terminal tab.

See [Terminal](../features/terminal.md#custom-command-shortcuts).

## AI

- **Commit and Push** — choose the provider CLI and edit the instructions used to generate the commit message. Muxy always stages all changes, commits, and pushes; the prompt cannot change that workflow.
- **Create Pull Request** — choose the provider CLI and edit the global instructions used to generate the pull request title, summary, new branch name, and target branch. The Create PR dropdown in the status bar can override this prompt for the current project. Muxy validates the response before creating the branch, commit, push, and pull request.
- **Auto** selects the first available supported provider CLI. Claude Code, Codex, Cursor Agent, Droid, Grok, OpenCode, and Pi are supported.
- Provider CLIs run non-interactively in the active worktree and return metadata without opening a terminal tab. Muxy disables provider write tools and performs every Git and GitHub mutation through its native services.
- Global prompts are stored as settings, while Create PR project overrides are stored with their project. Repository context such as changed paths and capped diffs is sent to the selected provider CLI. Do not put credentials or other secrets in prompts or changes sent to a remote provider.
- Model selection is not exposed yet. Configure a provider's own defaults when a specific model is required.

## Voice

- **Press Return after inserting** — sends dictated text immediately.
- **Language** — on-device speech recognition language.

See [Voice Recording](../features/voice-recording.md).

## Notifications

- **Toast** — show an in-app toast on arrival.
- **Desktop notifications** — show a macOS notification when Muxy is not frontmost.
- **Toast position** and **Sound** — delivery presentation.
- **AI Providers** — enable or disable hook integrations for Claude Code, Codex, Cursor, Droid, Grok, OpenCode, and Pi.
- **Per-source delivery** — separate toggles for provider hooks, OSC sequences, and the socket API.

See [Notifications](../features/notifications.md).

## Mobile

- **Allow Mobile Connections** — start or stop the WebSocket server.
- **Port** — defaults to 4865 in release builds.
- **Pair Mobile Device** — shows the pairing QR code.
- **Approved devices** — list of paired clients with revoke buttons.

See [Remote Server](../remote-server/overview.md).

## Backup

- Create, restore, and manage Muxy backups.
- Backups include settings, projects, worktrees, workspaces, remote devices, key bindings, command shortcuts, extension shortcuts, editor settings, and Ghostty config.

## JSON

The JSON tab exposes editable settings as `settings.json`.

Use it for bulk edits, sharing settings, or editing values faster than clicking through controls. Muxy validates the file before applying it.
