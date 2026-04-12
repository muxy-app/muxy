# Architecture

Muxy is a macOS terminal multiplexer built with SwiftUI that uses libghostty for terminal emulation.

## Directory Map

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
    AppState.swift            @Observable root state, dispatches workspace actions
    WorkspaceReducer.swift    Pure reducer: all workspace state transitions
    WorkspaceSnapshot.swift   Save/restore workspace layout to disk
    SplitNode.swift           Recursive binary tree for pane splits
    TabArea.swift             Container for tabs within a single pane
    TerminalTab.swift         Terminal or VCS tab model
    TabDragCoordinator.swift  Cross-pane tab drag-and-drop, TabMoveRequest, SplitPlacement
    KeyBinding.swift          ShortcutAction enum + KeyBinding defaults
    KeyCombo.swift            Key combo encoding, display, matching
    VCSTabState.swift         Git diff viewer state + loading orchestration
    EditorTabState.swift      Code editor tab state (backing store, cursor, search, save)
    EditorSettings.swift      @Observable editor preferences (font, wrap, tab size, feature toggles)
    TextBackingStore.swift    Line-array backing store for editor documents
    ViewportState.swift       Viewport window computation and line mapping for editor documents
    Project.swift             Project folder metadata
    Worktree.swift            Per-project worktree slot (primary or git worktree)
    WorktreeKey.swift         Hashable (projectID, worktreeID) key for workspace maps
    WorktreeConfig.swift      Decoder for .muxy/worktree.json setup commands
    TerminalPaneState.swift   Per-pane terminal state
    TerminalSearchState.swift Terminal find-in-page state
  Services/
    GhosttyService.swift      Singleton managing ghostty_app_t lifecycle
    GhosttyRuntimeEventAdapter.swift  C callback bridge from libghostty
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
    ThemeService.swift        Theme discovery + application
    MuxyConfig.swift          Ghostty config file read/write
    KeyBindingStore.swift     @Observable store for keyboard shortcuts
    KeyBindingPersistence.swift  JSON persistence for shortcuts
    ProjectStore.swift        @Observable store for projects list
    ProjectPersistence.swift  JSON persistence for projects
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
    MainWindow.swift          Main window layout (sidebar + workspace)
    Sidebar.swift             Project list sidebar
    Sidebar/
      ProjectRow.swift          Avatar + label row, worktree-chevron trigger on active row, ProjectAvatar
      WorktreePopover.swift     Worktree picker popover triggered from the active project row
      CreateWorktreeSheet.swift Sheet for creating a new git worktree
    ThemePicker.swift         Theme selection popover
    WelcomeView.swift         Empty state view
    Components/
      IconButton.swift        Reusable icon button
      FileDiffIcon.swift      Git diff file icon (SVG shape)
      WindowDragView.swift    NSView for window title bar dragging
      MiddleClickView.swift   NSView for middle-click tab close
      UUIDFramePreferenceKey.swift  Generic PreferenceKey for frame tracking
      QuickOpenOverlay.swift  Cmd+P file search overlay (name substring match via find)
    Terminal/
      GhosttyTerminalNSView.swift       AppKit view wrapping ghostty_surface_t + NSTextInputClient
      TerminalPane.swift      SwiftUI wrapper for terminal + search
      TerminalSearchBar.swift Find-in-terminal UI
      TerminalViewRegistry.swift  Terminal view lifecycle management
    Editor/
      CodeEditorRepresentable.swift  NSViewRepresentable bridge for code editor (viewport rendering path)
      EditorPane.swift        SwiftUI wrapper for editor tab (breadcrumb + editor)
      Extensions/
        SyntaxHighlightExtension.swift  Regex-based syntax highlighting rules for code editor
    VCS/
      VCSTabView.swift        Source control tab (commit, stage, diff, branch) + PRPill + PRPopover
      BranchPicker.swift      Branch selection dropdown with filter and right-click delete
      UnifiedDiffView.swift   Unified diff rendering
      SplitDiffView.swift     Side-by-side diff rendering
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
      AppearanceSettingsView.swift  Theme settings tab
      EditorSettingsView.swift  Editor preferences tab (font, wrap, tab size, feature toggles)
      KeyboardShortcutsSettingsView.swift  Shortcut config tab
      ShortcutRecorderView.swift  Shortcut capture field
      ShortcutBadge.swift     Shortcut label display
```

## Hierarchy

```
Project → Worktree → SplitNode (splits/tab areas) → TerminalTab → Pane
```

Each project has at least one **primary** worktree pointing at `Project.path`. Git
projects may add more worktrees via `git worktree add`, each with their own split
tree, tabs, focus state, and working directory. Workspace state is keyed by
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

- **Editor Pipeline**: All opened files are loaded into `TextBackingStore` and rendered through the viewport pipeline in `CodeEditorRepresentable`. The size thresholds in `EditorTabState` control only large-file warning/refusal UX, not editor mode selection.
- **GhosttyKit**: C module wrapping `ghostty.h`. Precompiled xcframework from `muxy-app/ghostty` fork. Surfaces created/destroyed via `TerminalViewRegistry`.
- **Persistence**: All files in `~/Library/Application Support/Muxy/`. Shared directory helper: `MuxyFileStorage`. Worktrees are persisted per-project at `worktrees/{projectID}.json`. Worktree setup commands live in-repo at `{Project.path}/.muxy/worktree.json`.
- **Ghostty Config**: Managed by `MuxyConfig`, stored at `~/Library/Application Support/Muxy/ghostty.conf`. Seeded from `~/.config/ghostty/config` on first run.
- **Updates**: Sparkle framework via `UpdateService`.

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
