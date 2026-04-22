# Architecture

Muxy is a macOS terminal multiplexer built with SwiftUI that uses libghostty for terminal emulation.
It is structured as a monorepo with a companion iOS app (MuxyMobile) that connects to the
desktop app over the local network.

## Monorepo Structure

```
MuxyShared/                    Shared types (macOS + iOS): protocol DTOs, messages, codec
  ProjectDTO.swift             Project data transfer object
  WorktreeDTO.swift            Worktree data transfer object
  WorkspaceDTO.swift           Workspace layout DTOs (SplitNodeDTO, TabAreaDTO, TabDTO)
  NotificationDTO.swift        Notification data transfer object
  VCSStatusDTO.swift           Git status/file DTOs
  MuxyProtocol.swift           Protocol enums: methods, results, events
  ProtocolParams.swift         Request parameter types for each method
  MuxyMessage.swift            Message envelope (request/response/event) + JSON codec

MuxyServer/                    WebSocket server library (macOS only, embedded in Muxy.app)
  MuxyRemoteServer.swift       NWListener-based WebSocket server + delegate protocol + request routing
  ClientConnection.swift       Per-client NWConnection wrapper, WebSocket framing

MuxyMobile/                    iOS companion app
  MuxyMobileApp.swift          App entry point
  ContentView.swift            Root view (connection state router)
  ConnectView.swift            Host/port connection form
  RemoteWorkspaceView.swift    Project list + workspace detail
  ConnectionManager.swift      WebSocket client, state sync, request/response handling
  DeviceCredentialsStore.swift Persistent deviceID + token stored in iOS Keychain
```

## Desktop App Directory Map

```
Muxy/
  MuxyApp.swift              App entry point, delegate, window setup
  Commands/
    MuxyCommands.swift        macOS menu bar commands
  Extensions/
    BundleExtension.swift     Bundle helper
    Notification+Names.swift  Custom notification names
    View+KeyboardShortcut.swift  .shortcut(for:store:) View extension
  Models/
    MuxyNotification.swift    Notification data model (pane, project, worktree IDs, source, content)
    AppState.swift            @Observable root state, dispatches workspace actions
    WorkspaceReducer.swift    Pure reducer: all workspace state transitions
    WorkspaceSnapshot.swift   Save/restore workspace layout to disk
    NavigationHistory.swift   Stacked back/forward history over project+worktree+area+tab tuples
    SplitNode.swift           Recursive binary tree for pane splits
    TabArea.swift             Container for tabs within a single pane
    TerminalTab.swift         Terminal, VCS, editor, or diff-viewer tab model
    TabDragCoordinator.swift  Cross-pane tab drag-and-drop, TabMoveRequest, SplitPlacement
    KeyBinding.swift          ShortcutAction enum + KeyBinding defaults
    KeyCombo.swift            Key combo encoding, display, matching
    VCSTabState.swift         Git diff viewer state + loading orchestration
    EditorTabState.swift      Code editor tab state (backing store, cursor, search, save)
    DiffViewerTabState.swift  Standalone diff-viewer tab state (single-file diff, unified/split toggle, session-only — not persisted)
    FileTreeState.swift       Lightweight file tree state per worktree (lazy expansion, git statuses)
    EditorSettings.swift      @Observable editor preferences (default editor, font)
    TextBackingStore.swift    Line-array backing store for editor documents
    ViewportState.swift       Viewport window computation and line mapping for editor documents
    TerminalSettings.swift    Terminal preference keys and quick-select label layout helpers
    ProjectLifecyclePreferences.swift  Project lifecycle preferences (keep-open-when-no-tabs)
    Project.swift             Project folder metadata
    Worktree.swift            Per-project worktree slot (primary or git worktree)
    WorktreeKey.swift         Hashable (projectID, worktreeID) key for workspace maps
    WorktreeConfig.swift      Decoder for .muxy/worktree.json setup commands
    TerminalPaneState.swift   Per-pane terminal state, including startup commands for terminal editors
    TerminalSearchState.swift Terminal find-in-page state
    TerminalQuickSelectState.swift Keyboard quick-select match state and label generation
  Services/
    GhosttyService.swift      Singleton managing ghostty_app_t lifecycle
    GhosttyRuntimeEventAdapter.swift  C callback bridge from libghostty (OSC + command finished → notifications)
    NotificationStore.swift      @Observable notification store singleton (persisted to notifications.json)
    NotificationNavigator.swift  Pane context resolution + click-to-navigate dispatch
    NotificationSocketServer.swift  Unix domain socket IPC for external tool notifications
    Git/
      GitRepositoryService.swift  Git command execution (Sendable struct; dispatches via GitProcessRunner)
      GitProcessRunner.swift      Concurrent Process dispatcher for git/gh, unblocks main thread
      GitSignpost.swift           os_signpost helpers for instrumenting git/gh calls
      GitWorktreeService.swift    git worktree list/add/remove (actor)
      GitDiffParser.swift         Diff patch parsing, context collapsing
      GitStatusParser.swift       Porcelain + numstat output parsing
      GitModels.swift             GitStatusFile, DiffDisplayRow, NumstatEntry
    GitDirectoryWatcher.swift FSEvents watcher for .git changes
    FileSearchService.swift   Quick open file search via /usr/bin/find subprocess
    FileTreeService.swift     Lazy directory listing that respects .gitignore via git check-ignore
    FileSystemOperations.swift Off-main create / rename / move / copy / trash primitives
    FileClipboard.swift       NSPasteboard wrapper for file cut/copy/paste with cut-marker type
    ThemeService.swift        Theme discovery + application
    MuxyConfig.swift          Ghostty config file read/write
    KeyBindingStore.swift     @Observable store for keyboard shortcuts
    KeyBindingPersistence.swift  JSON persistence for shortcuts
    ProjectStore.swift        @Observable store for projects list
    ProjectPersistence.swift  JSON persistence for projects
    ApprovedDevicesStore.swift Approved mobile devices (deviceID, SHA-256 token hash), revocation
    PairingRequestCoordinator.swift Queues pending pairing requests for UI approval prompts
    MobileServerService.swift  Lifecycle wrapper around MuxyRemoteServer
    WorktreeStore.swift       @Observable store for per-project worktrees
    WorktreePersistence.swift JSON persistence for worktrees (one file per project)
    ProjectOpenService.swift  Shared open-project flow used by commands and sidebar
    WorktreeSetupRunner.swift Dispatches .muxy/worktree.json setup commands to a new tab
    WorkspacePersistence.swift JSON persistence for workspaces
    JSONFilePersistence.swift Shared App Support directory helper
    ModifierKeyMonitor.swift  Global modifier key state tracking
    UpdateService.swift       Sparkle update checker
    ShortcutContext.swift     Window focus context for shortcuts
    AppEnvironment.swift      Dependency injection container
    AppStateDependencies.swift Protocol definitions for DI
  Theme/
    MuxyTheme.swift           Color system derived from Ghostty palette
  Views/
    NotificationPanel.swift   Notification list popover (bell icon in sidebar footer)
    MainWindow.swift          Main window layout (sidebar + workspace)
    Sidebar.swift             Narrow icon-strip sidebar (44px), add-project button, project icons
    Sidebar/
      ProjectRow.swift          Project icon (first letter or emoji logo), tooltip, context menu with logo + color pickers
      ProjectIconColorPicker.swift  Preset color palette popover for tinting the default letter icon
      WorktreePopover.swift     Worktree picker popover triggered from the active project row
      CreateWorktreeSheet.swift Sheet for creating a new git worktree
    ThemePicker.swift         Theme selection popover (hosted in topbar right)
    WelcomeView.swift         Empty state view
    Components/
      IconButton.swift        Reusable icon button
      FileDiffIcon.swift      Git diff file icon (SVG shape)
      FileTreeIcon.swift      File tree toggle button (SF symbol)
      WindowDragView.swift    NSView for window title bar dragging
      MiddleClickView.swift   NSView for middle-click tab close
      UUIDFramePreferenceKey.swift  Generic PreferenceKey for frame tracking
      NotificationBadge.swift Unread count badge for sidebar project icons
      QuickOpenOverlay.swift  Cmd+P file search overlay (name substring match via find)
    Terminal/
      GhosttyTerminalNSView.swift       AppKit view wrapping ghostty_surface_t + NSTextInputClient
      TerminalPane.swift      SwiftUI wrapper for terminal, search, and quick-select overlays
      TerminalSearchBar.swift Find-in-terminal UI
      TerminalViewRegistry.swift  Terminal view lifecycle management
    Editor/
      CodeEditorRepresentable.swift  NSViewRepresentable bridge for code editor (viewport rendering path)
      EditorPane.swift        SwiftUI wrapper for editor tab (breadcrumb + editor)
    FileTree/
      FileTreeView.swift      Side panel rendering of the lightweight file tree
      FileTreeCommands.swift  Orchestrates create/rename/delete/cut/copy/paste/drop
    VCS/
      VCSTabView.swift        Source control tab (commit, stage, diff, branch) + PRPill + PRPopover
      BranchPicker.swift      Branch selection dropdown with filter and right-click delete
      UnifiedDiffView.swift   Unified diff rendering
      SplitDiffView.swift     Side-by-side diff rendering
      DiffViewerPane.swift    Standalone diff-viewer tab (top bar + unified/split switch)
      DiffComponents.swift    Shared diff UI: line rows, highlighting, cache
      CreatePRSheet.swift     Sheet for opening a pull request on the current branch
      CommitHistoryView.swift Commit history list with context menu actions
    Workspace/
      Workspace.swift         Workspace container (split tree root)
      PaneNode.swift          Recursive split pane rendering
      SplitContainer.swift    Split pane with resize handle
      TabAreaView.swift       Tab area wrapper (tabs + content)
      TabStrip.swift          Tab bar with drag reordering
      DropZoneOverlay.swift   Tab split-mode drop targets
    Settings/
      SettingsView.swift      Settings window layout
      SettingsComponents.swift  Shared section/row primitives used across all tabs
      AppearanceSettingsView.swift  Theme settings tab
      EditorSettingsView.swift  Editor preferences tab (default editor, font)
      TerminalSettingsView.swift  Terminal preferences tab, including quick-select label layout
      KeyboardShortcutsSettingsView.swift  Shortcut config tab
      NotificationSettingsView.swift  Notification preferences tab
      MobileSettingsView.swift  Mobile server and approved devices tab
      ShortcutRecorderView.swift  Shortcut capture field
      ShortcutBadge.swift     Shortcut label display
```

## Hierarchy

```
Project → Worktree → SplitNode (splits/tab areas) → TerminalTab → Pane
```

Each project has at least one **primary** worktree pointing at `Project.path`. Git
projects may add more worktrees via `git worktree add`, each with their own split
tree, tabs, focus state, and working directory. Secondary worktrees can be either
Muxy-managed checkouts created from the sidebar or externally created Git worktrees
that are imported into the sidebar with a manual refresh. Workspace state is keyed by
`WorktreeKey(projectID, worktreeID)` in `AppState` so every per-project map is
actually per-worktree. `AppState.activeWorktreeID[projectID]` tracks which
worktree is currently visible for each project.

## Data Flow

```
User action → AppState.dispatch() → WorkspaceReducer.reduce()
                                        ↓
                              WorkspaceState (immutable update)
                              WorkspaceSideEffects (pane create/destroy)
                                        ↓
                              AppState applies effects
                              TerminalViewRegistry creates/destroys surfaces
```

## Key Integration Points

- **Editor Pipeline**: File opening routes through `AppState.openFile`. `EditorSettings.defaultEditor`
  chooses either the built-in editor or a configured terminal command. Built-in editor tabs load files into
  `TextBackingStore` and render through `CodeEditorRepresentable`; terminal editor tabs create a normal
  terminal pane with the configured Ghostty startup command. The size thresholds in
  `EditorTabState` apply only to the built-in editor path.
- **GhosttyKit**: C module wrapping `ghostty.h`. Precompiled xcframework from `muxy-app/ghostty` fork. Surfaces created/destroyed via `TerminalViewRegistry`.
- **Persistence**: All files in `~/Library/Application Support/Muxy/`. Shared directory helper: `MuxyFileStorage`. Worktrees are persisted per-project at `worktrees/{projectID}.json`, including whether a secondary worktree is Muxy-managed or externally discovered. Git projects can manually refresh this list from `git worktree list --porcelain` to import existing worktrees without deleting absent entries; paths are matched after symlink resolution so a repo opened via a symlinked path still collapses onto a single primary entry. Externally discovered worktrees are never touched by Muxy's `cleanupOnDisk` paths (project removal, post-merge cleanup, manual removal) — they can only be unregistered by the user in the underlying repo. Worktree setup commands live in-repo at `{Project.path}/.muxy/worktree.json`.
- **Ghostty Config**: Managed by `MuxyConfig`, stored at `~/Library/Application Support/Muxy/ghostty.conf`. Seeded from `~/.config/ghostty/config` on first run.
- **Updates**: Sparkle framework via `UpdateService`.
- **Window Title**: `NSWindow.title` is hidden visually (`titleVisibility = .hidden`) but set
  reactively by `WindowTitleUpdater` in `MainWindow` to `{project name} — {active tab title}`
  (or just the project name if no tab title is known). This makes Muxy sessions identifiable
  to accessibility readers and activity trackers (e.g., ActivityWatch) that read `AXTitle`.
  Tab titles come from the active tab's `TerminalTab.title`, which follows OSC 0/2 updates
  via `GhosttyRuntimeEventAdapter` → `TerminalPaneState.setTitle`. Users can override the
  auto-title via `TerminalTab.customTitle` ("Rename Tab" context menu / `⌃⌘R`) and assign a
  color accent via `TerminalTab.colorID` ("Set Tab Color…" context menu). Both fields persist
  to `workspaces.json` through `TerminalTabSnapshot`. Colors resolve through
  `ProjectIconColor.palette` (shared with project icon colors).

## File Tree

The file tree is a lightweight side panel mounted at the trailing edge of the
main window, in the same slot used by the attached VCS panel. Only one of the
two panels can be visible at a time — opening one closes the other. Both are
toggled from buttons in the topbar (file tree button appears only when the VCS
display mode is `attached`, since the file tree panel reuses the attached slot).

`FileTreeState` is created per `WorktreeKey` and held by `MainWindow`. It lazily
loads directory contents through `FileTreeService.loadChildren`, which calls
`git check-ignore --stdin` for the candidate names in each directory so the
visible tree matches `.gitignore`. Non-git folders fall back to a hardcoded
prune list (same one used by `FileSearchService`).

Per-file git statuses come from `git status --porcelain=v1 -z` and are mapped
to colors (modified → diff hunk color, added/untracked → diff add color,
deleted/conflict → diff remove color). Parent directories of changed files are
highlighted with the modified color. The tree subscribes to
`.vcsRepoDidChange` and uses `GitDirectoryWatcher` so external changes refresh
the panel without user action — there is no manual refresh button. Clicking a
file routes through `AppState.openFile`, the same path used by the quick open
overlay.

The header has a filter button that toggles `showOnlyChanges`, hiding any
entry whose absolute path is not in the status set (and any directory whose
subtree has no changes). The panel also tracks the active editor file via
`AppState.activeTab(for:)?.content.editorState?.filePath`: changes to that path
auto-expand its parent directories and highlight the row using
`MuxyTheme.accentSoft`. Deleted paths that no longer exist on disk are
materialized as synthetic tree rows so removals still appear in both the full
tree and the changed-only filter.

The panel width is persisted in `UserDefaults` under `muxy.fileTreeWidth`.
Expansion state is in-memory only.

### File Operations

The tree supports direct manipulation through a right-click context menu,
keyboard shortcuts, and drag-and-drop. `FileTreeCommands` (held as view
state inside `FileTreeView`) orchestrates the flow: it mutates transient
`FileTreeState` fields (`pendingNewEntry`, `pendingRenamePath`,
`pendingDeletePaths`, `cutPaths`, `dropHighlightPath`, `selectedPaths`,
`selectionAnchorPath`) and dispatches work to `FileSystemOperations`, a
stateless service that runs create / rename / move / copy / trash off the
main thread via `GitProcessRunner.offMainThrowing`. Trash goes through
`NSWorkspace.shared.recycle` so the OS handles Undo.

Selection is multi-item: plain click selects one, `⌘`-click toggles, and
`⇧`-click extends the range using the currently visible row order.
Rename and new-entry both use `FileTreeRenameField`, an inline text field
that commits on Return / blur and cancels on Escape. Errors from any
operation surface through `ToastState.shared` and are also logged.

Cut / copy / paste is backed by `FileClipboard`, which writes file URLs to
`NSPasteboard.general` and tags cuts with a private pasteboard type
(`app.muxy.fileCut`). This lets Muxy round-trip cut state while remaining
interoperable with Finder (which only sees the file URLs). Paste into a
file selects that file's parent directory as the destination.

Drag-and-drop accepts `.fileURL` providers on every directory row and on
the empty space below the tree. Holding Option turns a move into a copy;
drops that would move a path into itself are filtered out. The dragged
row and all drop targets are driven by the same `FileTreeDropDelegate`.

When a path changes on disk (rename, move, paste) the tree calls
`AppState.handleFileMoved(from:to:)`, which walks every open editor tab
and rewrites `EditorTabState.filePath` — both exact matches and paths
under a moved directory — keeping editors pointed at the same content.
"Open in Terminal" dispatches `.createTabInDirectory`, a reducer case
that opens a new terminal tab rooted at the selected directory rather
than the project root.

## VCS Tab Layout

The VCS tab is organized top-to-bottom as:

1. **Header** — worktree trigger, branch picker, `PRPill`, settings, refresh.
2. **Commit area** — commit message field + three first-class buttons: `Commit`, `Pull` (with `↓N` badge when behind), `Push` (with `↑N` badge when ahead). Commit hotkey is `⌘↵`.
3. **Sections** — Staged / Changes / History resizable split.

Pull request management lives entirely in the header via `PRPill`, not in the commit area. `PRPill` renders one of the states from `VCSTabState.PRLaunchState`:

- `hidden` — nothing to PR (clean tree on default branch, or loading). Pill is not rendered.
- `ghMissing` — disabled pill prompting to install `gh`.
- `canCreate` — "Create PR" button that opens `CreatePRSheet`.
- `hasPR(info)` — pill opens `PRPopover` showing state, base branch, mergeability, and actions (Open on GitHub, Merge, Close, Refresh).

`canCreate` is gated by `VCSTabState.canCreatePR`: shown when `gh` is installed, no PR exists for this branch, and either the working tree has changes OR the current branch differs from the default branch.

`CreatePRSheet` drives the end-to-end flow via a `PRCreateRequest` passed to `VCSTabState.openPullRequest`:

1. **Target branch** — picked from `GitRepositoryService.listRemoteBranches` (remote-only), pre-selecting the repo's default branch.
2. **Title + description** — entered by the user; both fields start blank.
3. **Branch strategy** — radio between "use current branch" (hidden when on the default branch or when current == target) and "create new branch" (starts blank, then auto-slugs from the title until the user edits the name manually).
4. **Include** — radio between "all changes" (default) and "only staged"; hidden when there are no changes or only one kind.
5. **Draft** — checkbox that adds `--draft` to `gh pr create`.

On submit, `performPRFlow` runs: optional branch create+switch → optional stage (all if include=all, staged-only otherwise) → commit with title if anything is staged → `git push -u origin <branch>` → `gh pr create`. No rollback on partial failure — errors surface to the sheet with a clear message so the user can retry manually from wherever the flow stopped. Ahead/behind counts are populated by `GitRepositoryService.aheadBehind` during refresh and drive the push/pull badges in the commit area.

## Navigation History

`AppState` owns a `NavigationHistory` that captures a stacked history of
user navigation across projects, worktrees, split areas, and tabs. Each
entry is a `(projectID, worktreeID, areaID, tabID)` tuple. After every
successful `dispatch`, the current tuple is recorded (deduping against the
top of the stack). Selecting a different project, switching worktrees,
focusing another split pane, or selecting a different tab all count as
navigation events.

Back/forward navigation is exposed via `AppState.goBack()` /
`AppState.goForward()`. Both validate the target entry still references
live state (the worktree root is still in `workspaceRoots`, the area and
tab still exist) and transparently skip stale entries. The single state
transition is driven through the reducer via a dedicated
`Action.navigate(projectID:worktreeID:areaID:tabID:)` case so all
workspace mutations stay in the reducer. Re-recording during a
back/forward step is gated by
`NavigationHistory.performWithRecordingSuppressed`. After every dispatch
the history is swept: entries whose project, worktree, area, or tab no
longer exist are removed eagerly, and the cursor snaps to the post-reducer
active tuple when it is still present — so closing a tab simply takes
that entry out of the stack rather than leaving a stale hop.

The topbar hosts two chevron buttons (to the right of the sidebar border)
wired to these calls. Keyboard (default `⌃⌘←` / `⌃⌘→`), mouse side
buttons (buttons 3/4), and horizontal swipe gestures (Magic Mouse
1-finger, 3-finger trackpad) all trigger the same actions. The main
window's shortcut interceptor installs a local `addLocalMonitorForEvents`
handler for `[.otherMouseDown, .swipe]`, gated on the monitored window
being key and identified as a Muxy main window.

## Notification System

Notifications alert users when terminal events occur (command completion, AI agent
messages, OSC escape sequences). Each notification carries full navigation context
(projectID, worktreeID, areaID, tabID) to enable click-to-focus on the originating pane.

### Sources

- **OSC 9/777** — Desktop notification escape sequences handled via
  `GHOSTTY_ACTION_DESKTOP_NOTIFICATION` in `GhosttyRuntimeEventAdapter`.
- **Claude Code hooks** — Rich notifications from Claude Code sessions via a wrapper
  script that injects `--hooks` to route lifecycle events through the Unix socket.
- **Unix socket** — External tool integration via `~/Library/Application Support/Muxy/muxy.sock`. Accepts
  pipe-delimited messages with paneID for routing.

### Data Flow

```
Terminal event → GhosttyRuntimeEventAdapter / NotificationSocketServer
     → TerminalViewRegistry.paneID(for:) (reverse lookup)
     → NotificationNavigator.resolveContext() (pane → project/worktree/area/tab)
     → NotificationStore.add() (suppressed if pane is focused and app active)
     → Toast + sound delivery
     → Persist to notifications.json (debounced)
     → UI update (badge on sidebar, notification panel)
```

### Environment Variables

Each terminal surface receives `MUXY_PANE_ID`, `MUXY_PROJECT_ID`,
`MUXY_WORKTREE_ID`, and `MUXY_SOCKET_PATH` via `ghostty_surface_config_s.env_vars`.
These are used by the Claude wrapper script and socket API to identify the
originating pane.

### Click-to-Navigate

`NotificationNavigator.navigate(to:)` dispatches three `AppState` actions in
sequence: `selectProject` → `focusArea` → `selectTab`. System notifications encode
the navigation context in `userInfo` and bring the app to front on click.

## Remote Server (MuxyServer)

The desktop app embeds a WebSocket server (`MuxyRemoteServer`) that exposes
workspace state and terminal operations to the iOS companion app over the local
network (LAN, Tailscale, etc.).

### Architecture

```
MuxyMobile (iOS)  ◄── WebSocket (JSON) ──►  MuxyRemoteServer (inside Muxy.app)
                                                    │
                                                    ▼
                                             MuxyRemoteServerDelegate
                                             (AppState, ProjectStore, etc.)
```

The server listens on a user-configurable port (default 4865) when enabled in
Mobile settings. The port is stored in `UserDefaults` and applied on start.
`MobileServerService` reports bind failures back to the UI: if the listener
fails to start (e.g. port in use), the enable toggle is rolled off and the
settings view displays the error. It uses Apple's Network framework
(`NWListener` + `NWConnection`) with the WebSocket protocol. All messages use
the `MuxyMessage` JSON envelope from `MuxyShared`.

### Protocol

Request-response with server-pushed events:

- **Request/Response** — Client sends `MuxyRequest` (method + params), server
  replies with `MuxyResponse` (result or error). Each request has a unique ID
  for correlation.
- **Events** — Server pushes `MuxyEvent` to all connected clients when state
  changes (workspace updates, new notifications, project list changes).

### Shared Types (MuxyShared)

Platform-agnostic DTOs used by both apps. All types are `Codable` and `Sendable`.
The `MuxyCodec` handles JSON encoding/decoding with ISO 8601 dates.

### iOS App (MuxyMobile)

`ConnectionManager` manages the WebSocket lifecycle and maintains a local mirror
of the remote state (projects, workspace layout, notifications). It also keeps a
rolling connection trace so mobile failures can surface a user-shareable
technical report from the phone's error sheet. `TerminalView` renders the
remote terminal grid locally, sends input back over the socket, and freezes the
current snapshot during long-press text selection so copy actions operate on a
stable view. Views observe this state and dispatch actions back through the
connection.

### Device Pairing

Connections are gated by a trust-on-first-use pairing handshake. Each mobile
device generates a persistent `deviceID` (UUID) and a random `token` on first
launch; both are stored in the iOS Keychain (`DeviceCredentialsStore`).

On every connect, the mobile app sends `authenticateDevice` first. The Mac
(`ApprovedDevicesStore`) compares the device's SHA-256 token hash against the
stored hash for that `deviceID`:

- **Known device with matching token** → immediately authorized.
- **Unknown device** → server returns `401 Unauthorized`. Mobile falls back to
  `pairDevice`, and `PairingRequestCoordinator` on the Mac queues the request
  and surfaces an approval sheet on `MainWindow`. Approval stores the token
  hash in `~/Library/Application Support/Muxy/approved-devices.json`; denial
  returns `403`.
- **Token mismatch** → treated the same as unknown; server returns `401` so a
  stolen but outdated credential can't resume authentication.

Until the handshake succeeds the server rejects every other RPC with
`401 Unauthorized`. After success, the client is added to an
`authenticatedClients` set on `MuxyRemoteServer`; broadcasts only go to clients
in that set. The `Mobile` tab in Settings lists approved devices with a Revoke
action, which removes the device from storage and terminates any active
connection for that `deviceID` via `MuxyRemoteServer.disconnect(deviceID:)`.
