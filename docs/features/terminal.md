# Terminal

Muxy's terminals are powered by [libghostty](https://github.com/ghostty-org/ghostty), running on a Metal layer for fast, GPU-accelerated rendering.

## Backend architecture

Muxy currently ships Ghostty as its terminal backend. Pane hosting, remote control, quick terminal creation, search, rich input, process detection, and offline lifecycle depend on Muxy's backend-neutral terminal surface contract. Optional integrations use dedicated capability protocols, so unsupported search, remote snapshots, client themes, offline lifecycle, raw output, and attachment upload behavior is never invoked. Image and file attachments fall back to escaped local file paths when a backend does not support attachment uploads. Ghostty-specific handles and callbacks stay inside the Ghostty implementation boundary. There is no user-facing backend selector until another implementation satisfies the capabilities required by these integrations.

## Background sessions

**Settings → Terminal → Background sessions** preserves local terminal sessions across an app restart or crash. It is off by default. Changing it presents **Restart Now** and **Cancel**, and the new value takes effect only after the whole app restarts. Remote terminals and Quick Terminal are excluded.

Every persistent session belongs to exactly one terminal tab and uses that tab's stable uppercase UUID. The Rust `muxy-session-v2` daemon owns the PTY and a bounded 256 KiB replay buffer while the in-app Ghostty surface connects through its private attach client. The daemon and app use a Rust-owned `MXS2` protocol and a versioned socket namespace selected from the active Rust profile; Swift sessions are not compatible peers.

### Recovering sessions

On startup Muxy reattaches every recoverable local terminal tab, including hidden tabs and panes. Output produced while the app was absent is replayed before live output, and the same shell continues accepting input. An unexpected daemon descriptor without a surviving tab is restored into a normal fallback terminal tab.

Recovery never removes or restructures a tab. A transport failure shows **Reconnect**. Only a daemon-confirmed missing session shows **Start Fresh**, which starts a new shell with the same tab identity and latest saved working directory. Closing a persistent tab first terminates its complete shell session and removes the tab only after a safe acknowledgement. Quitting or crashing Muxy does not terminate sessions.

Turning the feature off terminates all Rust sessions before persisting the setting. Projects, worktrees, tab and split layout, and latest working directories remain; after restart the tabs open fresh ordinary shells. Persistent sessions have no public CLI commands, status item, manual reassignment flow, or tabless workflow.

## Idle terminal freeing

**Settings → Terminal → Memory → Free idle inactive terminals** is also off by default, but its toggle and timeout apply live without restarting Muxy. A terminal's idle clock runs whenever it is not both visible on screen and focused. Muxy preserves terminals with a running command, unknown activity, or an alternate-screen program, and never frees remote terminals or Quick Terminal.

Freeing drops the in-app Ghostty surface. Returning focus, selecting **Wake**, typing, searching, scrolling, or other external I/O recreates it in the latest known working directory. An ordinary terminal starts a fresh shell and loses its old scrollback. A persistent terminal reconnects to its existing shell and receives bounded replay. A one-shot startup command is never run again on wake. Turning idle freeing off cancels its scan and wakes every freed terminal. Worktree-wide offline aggregation remains deferred to P10.

## Quick terminal

Assign Double Shift or a conventional global shortcut to open the quick terminal from anywhere. It opens centered near the top of the pointer's current display with a small gap below the usable screen edge. It always starts in your home directory and keeps the same shell, working directory, and history while hidden.

Dismiss it with the assigned shortcut, the close button, or a click outside the panel. Those paths hide the panel and retain its shell. `Cmd+W` closes the Quick Terminal and releases its surface; the next shortcut trigger starts a fresh shell. Moving the pointer and pressing Escape do not close it, so Escape reaches the terminal for `vim`, `less`, and other full-screen programs.

Quick Terminal has no shortcut assigned by default. Open **Settings → Quick Terminal** to choose one. System-wide Double Shift requires **System Settings → Privacy & Security → Input Monitoring**; conventional global shortcuts do not.

The same settings section can disable Quick Terminal entirely and controls the terminal width, height, transparency, and background vibrancy. Disabling it stops the shortcut listener, closes an open panel, and releases its shell. The shortcut and appearance settings remain saved; enabling it again starts a fresh shell. The gear button hides Quick Terminal and opens that settings section. Sizes are stored in points, constrained to 480–1200 wide and 280–800 high, and automatically reduced when the active display is smaller. Saved size and appearance changes apply the next time Quick Terminal opens.

Transparency continuously controls the terminal background tint from 0–55%. The vibrancy value is stored from 0–100%, but the current window-level renderer maps it to two states: 0 disables blur, while any nonzero value enables the same native blurred background. It is not a custom blur radius or a continuous material-strength control.

The panel and terminal surface are prepared at their final display-constrained size before they become visible. Show and hide reveal that fixed viewport through a rounded clipping animation instead of resizing the terminal during presentation. The tint is composited below Ghostty, preserving terminal glyph contrast. The bridge and controls follow the active terminal theme, while a native rounded window shadow provides the outer frame. Quick Terminal uses the separately configured light and dark terminal themes, updates a visible retained surface when the active theme changes, and switches live with macOS appearance. Muxy uses an opaque, unblurred fallback when macOS Reduce Transparency or Increase Contrast is enabled.

The quick terminal is available only on macOS while Muxy is running. Closing Muxy's main window quits the app and shuts down its shortcut listener, panel, and shell. Linux keeps the portable settings format but does not expose a Quick Terminal settings category or runtime.

## App transparency

**Settings → Interface → Appearance** brings the same transparency and vibrancy controls to the main window: terminal panes, the top bar, and the status bar. Transparency ranges from 0–55% and defaults to 0, keeping the window opaque until it is raised. Vibrancy mixes the native macOS material from 0–100% behind the transparent background; because the main window itself stays opaque, the desktop shows through the vibrancy material and the effect needs a vibrancy above zero. The sidebar keeps its own vibrancy toggle.

Changes apply immediately to open terminals, including split panes, and to the top bar and status bar. The active Ghostty theme is preserved: its background color is drawn as a tint over the material at the configured opacity, and colored cell backgrounds render normally. Muxy renders the workspace opaque and unblurred while macOS Reduce Transparency or Increase Contrast is enabled. A terminal pane controlled by a remote device keeps its opaque client theme until control returns to the Mac. The settings are stored as `muxy.app.transparency` and `muxy.app.blur` in `settings.json`.

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

### Attachments in SSH panes

A Mac file path does not resolve on a remote device, so an SSH pane uploads every attachment it receives and
inlines the remote path instead.

A pane counts as remote in two ways. A tab opened against a device configured under **Settings -> Remote Devices**
carries its destination directly. A pane where you typed `ssh` yourself is detected from the foreground process:
Muxy reads the running `ssh` invocation and reconstructs simple destinations using `user@host`, `-p`, `-l`, one
`-i`, or an `ssh://` URL. Config aliases are kept verbatim so `ssh_config` still resolves them. Relative identity
paths are resolved from the running SSH process's working directory. Uploads open their own connection to that
destination and share one multiplexed control socket, so a password-less key or agent is required. Invocations
with remote commands, multiple identities, or options Muxy cannot reproduce exactly are left alone and the pane
keeps local-path behavior. This includes `-J`, `-F`, `-W`, `-P`, `ProxyJump`, `ProxyCommand`, and unrepresented
`-o` settings. Identity paths containing environment or percent-token expansion are also refused.

Muxy refuses a chained invocation such as `ssh bastion ssh target`. If another SSH session is started later from
inside an interactive remote shell, macOS still exposes only the first local SSH process. Muxy cannot identify the
later hop, so attachment upload in nested interactive SSH sessions is unsupported.

Muxy accepts attachments through four routes:

| Route | Accepts |
| --- | --- |
| `Ctrl+V`, `Cmd+V`, right-click Paste | Clipboard image data, or files copied in Finder |
| Drag and drop onto the pane | Files |
| Composer image attachments | Images |
| Composer file attachments | Files |

Clipboard image data is converted to PNG away from the main UI thread. Files are streamed as-is with their
extension preserved, so the receiving TUI still recognizes the type; the remote name is the upload identifier
rather than the original file name. Each upload lands in a private, session-scoped temporary directory on the
remote device, and the remote path is pasted into the running TUI, letting tools such as Codex and Claude Code
read the attachment without access to the Mac.

Encoded image input and converted PNG output are limited to 25 MB, and decoded images are limited to 64
megapixels. Other files, including empty files, are limited to 100 MB. The upload timeout scales with payload size.
Every attachment must be a regular file, so directories, device files, and unbounded streams are rejected with a
toast naming the file.

Uploaded directories and files use owner-only permissions. Partial uploads are removed when an upload is
interrupted, and the session directory is removed when its terminal ends. A central cleanup coordinator retains
outstanding cleanup for both open and already-removed panes. On app quit, Muxy waits up to five seconds for those
tasks before allowing termination to continue. Text paste behavior is unchanged.

The **Settings -> Terminal -> Composer -> Image Submission** strategy applies to local panes only. When a Composer
upload fails, Muxy withholds Return and clears every line it has already submitted, so a partial prompt is never
left in the TUI. A dropped or pasted batch inlines whichever files uploaded successfully.

Local panes inline the escaped local path for dropped and pasted files, and are unaffected by upload limits.

## Mouse

Plain left-click and drag selects terminal text. `⌘` and right-click are reserved for Muxy, and neither starts
nor changes a text selection:

| Gesture | Result |
| --- | --- |
| `⌘` + left-click | Opens the file path or link under the pointer |
| `⌘` + left-drag | Moves the pane to another area or split |
| Right-click | Opens the [right-click menu](#right-click-menu) |

A `⌘` + left-drag is decided when the gesture starts, so pressing or releasing `⌘` mid-drag never switches
between moving the pane and selecting text.

Programs that enable mouse reporting keep receiving right-click. Hold `Shift` while right-clicking such a program
to get Muxy's menu instead.

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

`⌘I` on macOS and `Alt+I` on other platforms toggle Composer for the active local terminal pane. Composer is always an in-window panel. It can sit on the right or bottom, resize from the workspace-facing edge, and switch between **Pinned**, which displaces the workspace, and **Floating**, which overlays it. Position, mode, size, broadcast state, and font size persist in the selected profile.

Composer follows pane and worktree selection. A pane change in the same worktree rebinds the open panel without replacing that worktree's draft. A worktree change transfers to that worktree's saved draft. Losing the target closes the panel. While Composer owns focus, ordinary native terminal input is suppressed; closing restores terminal focus when the target still exists.

The editor supports multiline text, Unicode selection, IME composition, undo/redo, cursor insertion, wrapping, scrolling, the configured editor font family, and the Composer line-height setting. Use **Attach files** to choose multiple local files. File chips preserve insertion order and can be removed independently. Clipboard paste prefers text, then file URLs, then an image. A copied image is stored privately under `RichInputImages/` and inserted as a stable `[Image N]` token. Dropped image paths remain ordinary file attachments and are not copied.

`⌘Return` submits with Return and `⌘⇧Return` submits without Return on macOS. The header also provides both send actions. When text is selected, submission uses that selected text. Local file paths are checked before publication and shell-escaped. Each pane receives one FIFO transaction, so bracketed text, images, rollback, and the optional Return cannot interleave with later keyboard input. Broadcast captures the visible active terminal panes in retained order and processes them sequentially even when an earlier pane fails.

Image submission is selected under **Settings → Composer → Image Submission**. **Clipboard Paste** temporarily replaces the native pasteboard, sends raw `Ctrl+V`, waits 300 ms, and restores every captured item and representation. **Inline File Path** inserts the escaped private PNG path in text/image order. Each unique image is normalized once per submission batch and reused across broadcast targets. A partial image-send failure clears only the transaction's partial input, appends no Return for that pane, continues later broadcast panes, and retains the draft and image for recovery.

**Clear After Sending** clears only after every target succeeds and only when the draft revision is unchanged. **Clear on Close** clears the released worktree draft on close or worktree transfer. Both are off by default. Drafts otherwise persist per worktree in the selected profile's private `rich-input-drafts.json`; startup removes only image files proven to have no durable reference.

External path drops use one shared parser. Composer attaches every accepted path in order and deduplicates against existing chips. A terminal drop focuses the routed pane and inserts shell-escaped paths joined by spaces without Return. The project sidebar accepts existing directories sequentially, selects an already-known project without duplicating it, adds/selects new directories, and silently ignores files.

Automated tests cover parser policy, native pasteboard/drop adapters, exact terminal bytes, persistence, failure retention, and isolated staged app lifecycle. Physical Finder delivery, visible drag highlighting, real clipboard timing in a TUI, visual quality, focus feel, and accessibility remain manual acceptance items.

The status-bar microphone remains available as the legacy voice recorder. It inserts the final transcript into
the control that was focused before recording and can optionally press Return afterward.

## Right-click menu

Inside a terminal pane: **Paste**, **Split Right**, **Split Left**, **Split Down**, **Split Up**, and **Terminal Settings…**. Terminal Settings opens Muxy's settings directly on the Terminal section.

Opening the menu never selects terminal text. While a program has mouse reporting enabled the right button belongs to that program, so hold `Shift` to open the menu instead.

Splitting creates a child pane inside the current top-level tab. Each pane keeps its own terminal, browser, source-control, or extension surface, while a one-pixel divider replaces the old per-pane tab strip. Child panes do not appear as separate entries in the window tab strip or the Tab Focused sidebar. An agent running in a child pane does appear as its own entry in the Agents Focused sidebar, and selecting it activates both the child pane and its parent top-level tab.

Dragging a top-level tab toward an edge docks the whole tab beside another top-level tab. Its child-pane layout moves with it and remains independent from the neighboring tab's child panes.

The Agents Focused layout keeps the normal top-level tab strip in the title bar and limits sidebar tab entries to detected AI agents, including idle sessions. An entry disappears as soon as its agent process exits, even when the tab keeps running a shell. Projects and worktrees remain visible when they have no agent sessions, and their add menu can start a new tab with any available agent provider. Clicking a project or worktree row activates it; clicking the already active row expands or collapses its agent list. When project sorting is set to Manual, local project headers can be dragged to reorder project blocks while their children remain grouped with the project. Agent rows can be dragged only within their current worktree and docked tab group without changing non-agent tab positions. A project with no tabs offers the same launchers as icons — a terminal plus one monochrome icon per installed provider — instead of the plain new-tab button. Tabs started from this menu appear immediately. Local launch attribution is confirmed by process detection and removed if the command exits before confirmation. Remote availability is checked through the configured SSH connection before the menu enables a provider.

## Notifications from the terminal

Muxy accepts the pinned Ghostty build's OSC 9 and OSC 777 desktop-notification actions. OSC 9 supplies a body and uses `Command executed!` when no title is present. OSC 777 supplies its title and body. The callback payload is copied immediately and routed as a transient terminal signal, so notification text never enters persisted terminal metadata.

When Muxy is active and the event targets the exact visible workspace pane, an OSC notification is sound-only. It does not create history, unread state, a toast, or a macOS notification. Every other accepted workspace-terminal OSC notification enters the shared newest-first history, which retains at most 200 records in the selected profile's private `notifications.json`. Quick Terminal has no workspace navigation target, so its accepted OSC notifications use transient toast, sound, and optional macOS delivery without creating history or unread state. Opening one activates Muxy without attempting workspace navigation.

The sidebar footer bell opens a 320 by 400 notification panel above its anchor. Rows show source, title, body, relative time, and unread state. Rows can be opened or removed, and the full history can be cleared. Live rows navigate by stable project, worktree, root-tab, area, and pane IDs. The stored worktree path is context data and does not override those IDs. A stale row recreates nothing but is still marked read. Loaded history remains available after restart and is marked read when the main window starts.

Notification toasts are controlled separately from generic success and error feedback. The latest toast replaces the previous one and dismisses after three seconds. macOS desktop delivery is optional and requests permission only when its setting is enabled. Ordinary delivery never opens the permission prompt. Sounds use the selected macOS system sound; `None` and unknown names are silent.

For AI coding agents, accepted Agent Hook v3 events use the same history and presentation path. Hook installation and agent lifecycle UI remain separate work. See [AI notifications](ai-notifications.md).

## Quick-select labels

Ghostty's quick-select feature lets you focus a pane or surface by typing a label key. Labels and bindings are configured in the Ghostty config.
