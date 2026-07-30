# Terminal

Muxy's terminals are powered by [libghostty](https://github.com/ghostty-org/ghostty), running on a Metal layer for fast, GPU-accelerated rendering.

## Backend architecture

Muxy currently ships Ghostty as its terminal backend. Pane hosting, remote control, quick terminal creation, search, rich input, process detection, and offline lifecycle depend on Muxy's backend-neutral terminal surface contract. Optional integrations use dedicated capability protocols, so unsupported search, remote snapshots, client themes, offline lifecycle, raw output, and image paste behavior is never invoked. Image attachments fall back to escaped file paths when a backend does not support clipboard image paste. Ghostty-specific handles and callbacks stay inside the Ghostty implementation boundary. There is no user-facing backend selector until another implementation satisfies the capabilities required by these integrations.

## Quick terminal

Assign Double Shift or a conventional global shortcut to open the quick terminal from anywhere. On a display with a camera cutout, it expands out of the cutout like a dynamic island. It always starts in your home directory and keeps the same shell, working directory, and history while hidden.

Dismiss it with the assigned shortcut or the close button. Moving the pointer, clicking another app, and pressing Escape do not close it, so Escape reaches the terminal for `vim`, `less`, and other full-screen programs. On a display without a camera cutout, the terminal opens at the same top-center position.

Quick Terminal has no shortcut assigned by default. Open **Settings → Quick Terminal** to choose one. System-wide Double Shift requires **System Settings → Privacy & Security → Input Monitoring**; conventional global shortcuts do not.

The same settings section can disable Quick Terminal entirely and controls the terminal width, height, transparency, and background vibrancy. Disabling it stops the shortcut listener, closes an open panel, and releases its shell. The shortcut and appearance settings remain saved; enabling it again starts a fresh shell. The in-place gear button provides the size and appearance controls while the terminal is running. Sizes are stored in points, constrained to 480–1200 wide and 280–800 high, and automatically reduced when the active display is smaller. Transparency ranges from 0–55%, and vibrancy uses a continuous 0–100% native material intensity.

Vibrancy controls how much of the native macOS material participates in the background composition. It does not set a custom blur radius, which AppKit does not expose for system materials.

Transparency and vibrancy apply only to the terminal workspace while preserving the active Ghostty theme. The cutout bridge and its controls stay solid for readability, and project terminals keep their own Ghostty configuration. Muxy uses an opaque, unblurred fallback when macOS Reduce Transparency or Increase Contrast is enabled.

The quick terminal is available while Muxy is running. Closing Muxy's main window still follows the existing quit behavior.

## Configuration

Muxy's active Ghostty config is `~/Library/Application Support/Muxy/ghostty.conf`. On first launch Muxy seeds it from `~/.config/ghostty/config` when that file exists; after that, Muxy reads and writes its own copy. Open it with **Muxy -> Open Configuration...**, reload after editing with `⌘⇧R`.

Most Ghostty options work — fonts, colors, padding, keybinds, shell integration. Muxy applies the active light/dark variant automatically when the system appearance changes.

### Chinese font rendering

Muxy maps common Chinese Unicode ranges to one font so Ghostty does not mix fallback faces within the same text. It uses the first configured `font-family` with broad Simplified Chinese, Traditional Chinese, and punctuation coverage; otherwise it uses the macOS system fallback.

Keep the Latin terminal font first and add the preferred Chinese font as a fallback:

```ini
font-family = JetBrains Mono
font-family = PingFang SC
```

Reload the configuration with `⌘⇧R`, then open a new terminal. Ghostty applies codepoint-map changes only to new terminals. Explicit `font-codepoint-map` entries in `ghostty.conf` take priority over Muxy's automatic mapping for overlapping ranges.

## Find in terminal

`⌘F` opens an inline search overlay scoped to the focused pane. Enter / Shift-Enter cycle through matches; Escape dismisses.

## Copy and paste

| Action | Shortcut |
| --- | --- |
| Copy (with selection) | `⌘C` |
| Send `^C` to program | `⌘C` with no selection |
| Paste | `⌘V` or right-click → Paste |
| X11 selection paste | Middle-click |

Enable **Settings -> Terminal -> Auto-copy terminal selection** to copy selected terminal text on mouse release.

When an SSH terminal receives an image through `Ctrl+V`, `Cmd+V`, right-click Paste, or Composer, Muxy converts
the image to PNG away from the main UI thread and uploads it to a private, session-scoped temporary directory on
the remote device. The remote file path is then pasted into the running TUI, allowing tools such as Codex and
Claude Code to attach the image without access to the Mac clipboard. Encoded image input and converted PNG output
are limited to 25 MB, and decoded images are limited to 64 megapixels. Composer image attachments must be regular
files and are read with a fixed 25 MB cap before decoding, so device files and unbounded streams are rejected.

Uploaded directories and images use owner-only permissions. Partial uploads are removed when an upload is
interrupted, and the session directory is removed when its terminal ends. A central cleanup coordinator retains
outstanding cleanup for both open and already-removed panes. On app quit, Muxy waits up to five seconds for those
tasks before allowing termination to continue. Text paste behavior is unchanged.

The **Settings -> Terminal -> Composer -> Image Submission** strategy applies to local panes only. SSH panes always
upload the image and inline its remote path, because a Mac file path does not resolve on the remote device. When an
upload fails, Muxy withholds Return and clears every line it has already submitted, so a partial prompt is never
left in the TUI.

## Working directory

Muxy tracks the cwd via Ghostty's shell integration (OSC 7). The directory is persisted in workspace snapshots so newly recreated tabs land in the same folder when applicable.

Remote terminals use the selected SSH device's environment before starting the remote login shell. New SSH devices default to `TERM=xterm-256color`; edit the device in Settings -> Remote Devices to change or remove it.

## Muxy CLI

Use the `muxy` command to open projects and control panes from a shell or automation script. See [Muxy CLI](muxy-cli.md).

## Custom command shortcuts

Define reusable shell command shortcuts in **Settings → Commands**:

- Display name, command, optional icon, optional keybinding.
- Triggering one creates a new tab and runs the command.
- Useful for `npm run dev`, `make watch`, `just test`, …

## Composer

`Cmd+I` opens the focused composer for multiline prompts, files, images, and broadcast sends. `Cmd+Shift+I`
opens the legacy voice recorder normally, or starts on-device dictation inside the focused composer when it is
already open. Stop Composer dictation to insert the transcript at the editor cursor, or press Return while
recording, then edit or send it normally.

Each Composer submission is serialized with later keyboard input for its target terminal. Text, image paths, and
the optional Return are submitted as one transaction, including when a Composer message is broadcast to several
panes. Broadcast targets are processed one at a time, and each unique image attachment is normalized once into
validated immutable PNG data that is reused across those targets.

Composer submission controls stay unavailable while dictation is starting or recording. A dictation error does
not block typed text from being sent, and its inline message can be dismissed without closing the composer.
Only one focused overlay is shown at a time, so Composer shortcuts do not open it over another modal.

The status-bar microphone remains available as the legacy voice recorder. It inserts the final transcript into
the control that was focused before recording and can optionally press Return afterward.

## Right-click menu

Inside a terminal pane: **Paste**, **Split Right**, **Split Left**, **Split Down**, **Split Up**, and **Terminal Settings…**. Terminal Settings opens Muxy's settings directly on the Terminal section.

Splitting creates a child pane inside the current top-level tab. Each pane keeps its own terminal, browser, source-control, or extension surface, while a one-pixel divider replaces the old per-pane tab strip. Child panes do not appear as separate entries in the window tab strip or the Tab Focused sidebar. An agent running in a child pane does appear as its own entry in the Agents Focused sidebar, and selecting it activates both the child pane and its parent top-level tab.

Dragging a top-level tab toward an edge docks the whole tab beside another top-level tab. Its child-pane layout moves with it and remains independent from the neighboring tab's child panes.

The Agents Focused layout keeps the normal top-level tab strip in the title bar and limits sidebar tab entries to detected AI agents, including idle sessions. An entry disappears as soon as its agent process exits, even when the tab keeps running a shell. Projects and worktrees remain visible when they have no agent sessions, and their add menu can start a new tab with any available agent provider. Clicking a project or worktree row activates it; clicking the already active row expands or collapses its agent list. A project with no tabs offers the same launchers as icons — a terminal plus one monochrome icon per installed provider — instead of the plain new-tab button. Tabs started from this menu appear immediately. Local launch attribution is confirmed by process detection and removed if the command exits before confirmation. Remote availability is checked through the configured SSH connection before the menu enables a provider.

## Notifications from the terminal

OSC 9 and OSC 777 notification escape sequences are routed into Muxy's notification panel and (optionally) macOS notifications.

For AI coding agents (Claude Code, Codex, Cursor, Droid, Grok, OpenCode, Pi), Muxy uses hook-based lifecycle events rather than escape sequences — see [AI notifications](ai-notifications.md).

## Quick-select labels

Ghostty's quick-select feature lets you focus a pane or surface by typing a label key. Labels and bindings are configured in the Ghostty config.
