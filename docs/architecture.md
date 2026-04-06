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
    Project.swift             Project folder metadata
    TerminalPaneState.swift   Per-pane terminal state
    TerminalSearchState.swift Terminal find-in-page state
  Services/
    GhosttyService.swift      Singleton managing ghostty_app_t lifecycle
    GhosttyRuntimeEventAdapter.swift  C callback bridge from libghostty
    Git/
      GitRepositoryService.swift  Git command execution (actor)
      GitDiffParser.swift         Diff patch parsing, context collapsing
      GitStatusParser.swift       Porcelain + numstat output parsing
      GitModels.swift             GitStatusFile, DiffDisplayRow, NumstatEntry
    GitDirectoryWatcher.swift FSEvents watcher for .git changes
    ThemeService.swift        Theme discovery + application
    MuxyConfig.swift          Ghostty config file read/write
    KeyBindingStore.swift     @Observable store for keyboard shortcuts
    KeyBindingPersistence.swift  JSON persistence for shortcuts
    ProjectStore.swift        @Observable store for projects list
    ProjectPersistence.swift  JSON persistence for projects
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
    ThemePicker.swift         Theme selection popover
    WelcomeView.swift         Empty state view
    Components/
      IconButton.swift        Reusable icon button
      FileDiffIcon.swift      Git diff file icon (SVG shape)
      WindowDragView.swift    NSView for window title bar dragging
      MiddleClickView.swift   NSView for middle-click tab close
      UUIDFramePreferenceKey.swift  Generic PreferenceKey for frame tracking
    Terminal/
      GhosttyTerminalNSView.swift       AppKit view wrapping ghostty_surface_t + NSTextInputClient
      TerminalPane.swift      SwiftUI wrapper for terminal + search
      TerminalSearchBar.swift Find-in-terminal UI
      TerminalViewRegistry.swift  Terminal view lifecycle management
    VCS/
      VCSTabView.swift        Source control tab (commit, stage, diff, branch)
      BranchPicker.swift      Branch selection dropdown with filter
      UnifiedDiffView.swift   Unified diff rendering
      SplitDiffView.swift     Side-by-side diff rendering
      DiffComponents.swift    Shared diff UI: line rows, highlighting, cache
      PRBadge.swift           Pull request link badge
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
      KeyboardShortcutsSettingsView.swift  Shortcut config tab
      ShortcutRecorderView.swift  Shortcut capture field
      ShortcutBadge.swift     Shortcut label display
```

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

- **GhosttyKit**: C module wrapping `ghostty.h`. Precompiled xcframework from `muxy-app/ghostty` fork. Surfaces created/destroyed via `TerminalViewRegistry`.
- **Persistence**: All files in `~/Library/Application Support/Muxy/`. Shared directory helper: `MuxyFileStorage`.
- **Ghostty Config**: Managed by `MuxyConfig`, stored at `~/Library/Application Support/Muxy/ghostty.conf`. Seeded from `~/.config/ghostty/config` on first run.
- **Updates**: Sparkle framework via `UpdateService`.
