import AppKit
import SwiftUI

struct MuxyCommands: Commands {
    @ObservedObject private var ideService = IDEIntegrationService.shared
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true

    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let projectGroupStore: ProjectGroupStore
    let keyBindings: KeyBindingStore
    let commandShortcuts: CommandShortcutStore
    let config: MuxyConfig
    let ghostty: GhosttyService
    let updateService: UpdateService

    private var isMainWindowFocused: Bool {
        ShortcutContext.isMainWindow(NSApp.keyWindow)
    }

    private var activeProject: Project? {
        guard let projectID = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == projectID }
    }

    private var activeProjectPath: String? {
        guard let project = activeProject else { return nil }
        return worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id])?.path
            ?? project.path
    }

    private var canCreateWorktreeForActiveProject: Bool {
        WorktreeActionEligibility.canCreateWorktree(
            project: activeProject,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            allowUnknownGitStatus: true
        )
    }

    private var canRemoveCurrentWorktree: Bool {
        WorktreeActionEligibility.removableCurrentWorktree(
            project: activeProject,
            appState: appState,
            worktreeStore: worktreeStore
        ) != nil
    }

    private var shortcutDispatcher: ShortcutActionDispatcher {
        ShortcutActionDispatcher(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            ghostty: ghostty
        )
    }

    private func performShortcutAction(_ action: ShortcutAction) {
        _ = shortcutDispatcher.perform(action, activeProject: activeProject)
    }

    private func performCommandShortcut(_ shortcut: CommandShortcut) {
        guard isMainWindowFocused,
              let projectID = appState.activeProjectID,
              appState.workspaceRoot(for: projectID) != nil,
              !shortcut.trimmedCommand.isEmpty
        else { return }
        appState.createCommandTab(projectID: projectID, shortcut: shortcut)
    }

    var body: some Commands {
        CommandGroup(replacing: .appSettings) {
            Button {
                NotificationCenter.default.post(name: .openSettingsModal, object: nil)
            } label: {
                Label(L10n.string("Settings..."), systemImage: "gearshape")
            }
            .keyboardShortcut(",", modifiers: .command)

            Button {
                NotificationCenter.default.post(name: .openExtensionsModal, object: nil)
            } label: {
                Label(L10n.string("Extensions..."), systemImage: "puzzlepiece.extension")
            }
            .keyboardShortcut(",", modifiers: [.command, .shift])
        }

        CommandGroup(after: .appSettings) {
            Button {
                NSWorkspace.shared.open(
                    [config.ghosttyConfigURL],
                    withApplicationAt: URL(fileURLWithPath: "/System/Applications/TextEdit.app"),
                    configuration: NSWorkspace.OpenConfiguration()
                )
            } label: {
                Label(L10n.string("Open Configuration..."), systemImage: "doc.text")
            }

            Button {
                performShortcutAction(.reloadConfig)
            } label: {
                Label(L10n.string("Reload Configuration"), systemImage: "arrow.clockwise")
            }
            .shortcut(for: .reloadConfig, store: keyBindings)

            Button {
                guard isMainWindowFocused else { return }
                performShortcutAction(.refreshWorktrees)
            } label: {
                Label(L10n.string("Refresh Worktrees"), systemImage: "arrow.triangle.2.circlepath")
            }
            .shortcut(for: .refreshWorktrees, store: keyBindings)
            .disabled(activeProject == nil)

            Button {
                guard isMainWindowFocused else { return }
                performShortcutAction(.createWorktree)
            } label: {
                Label(L10n.string("New Worktree"), systemImage: "plus")
            }
            .shortcut(for: .createWorktree, store: keyBindings)
            .disabled(!canCreateWorktreeForActiveProject)

            Button {
                guard isMainWindowFocused else { return }
                performShortcutAction(.removeCurrentWorktree)
            } label: {
                Label(L10n.string("Remove Current Worktree"), systemImage: "trash")
            }
            .shortcut(for: .removeCurrentWorktree, store: keyBindings)
            .disabled(!canRemoveCurrentWorktree)

            Divider()

            Button {
                CLIAccessor.installCLI()
            } label: {
                Label(L10n.string("Install CLI"), systemImage: "terminal")
            }

            Button {
                updateService.checkForUpdates()
            } label: {
                Label(L10n.string("Check for Updates..."), systemImage: "arrow.triangle.2.circlepath")
            }
            .disabled(!updateService.canCheckForUpdates)
        }

        CommandGroup(replacing: .pasteboard) {
            Button(L10n.string("Cut")) { NSApp.sendAction(#selector(NSText.cut(_:)), to: nil, from: nil) }
                .keyboardShortcut("x", modifiers: .command)
            Button(L10n.string("Copy")) { NSApp.sendAction(#selector(NSText.copy(_:)), to: nil, from: nil) }
                .keyboardShortcut("c", modifiers: .command)
            Button(L10n.string("Paste")) { NSApp.sendAction(#selector(NSText.paste(_:)), to: nil, from: nil) }
                .keyboardShortcut("v", modifiers: .command)
            Button(L10n.string("Select All")) { NSApp.sendAction(#selector(NSText.selectAll(_:)), to: nil, from: nil) }
                .keyboardShortcut("a", modifiers: .command)

            Divider()

            Button(L10n.string("Find")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.findInTerminal)
            }
            .shortcut(for: .findInTerminal, store: keyBindings)
        }

        CommandGroup(replacing: .newItem) {
            Button(L10n.string("Open Project...")) {
                performShortcutAction(.openProject)
            }
            .shortcut(for: .openProject, store: keyBindings)

            Menu(L10n.string("Open in IDE")) {
                Button {
                    guard let activeProjectPath else { return }
                    _ = ideService.openProject(at: activeProjectPath, in: IDEIntegrationService.finderApplication)
                } label: {
                    HStack(spacing: 8) {
                        AppBundleIconView(appURL: IDEIntegrationService.finderAppURL, fallbackSystemName: "folder", size: 20)
                        Text(L10n.resource("Finder"))
                    }
                }

                Divider()

                if ideService.installedApps.isEmpty {
                    Button(L10n.string("No supported IDEs found")) {}
                        .disabled(true)
                } else {
                    ForEach(ideService.installedApps) { ide in
                        Button {
                            guard let activeProjectPath else { return }
                            _ = ideService.openProject(at: activeProjectPath, in: ide)
                        } label: {
                            HStack(spacing: 8) {
                                AppBundleIconView(appURL: ide.appURL, fallbackSystemName: ide.symbolName, size: 20)
                                Text(ide.displayName)
                            }
                        }
                    }
                }
            }
            .disabled(activeProjectPath == nil)

            Button(L10n.string("New Tab")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.newTab)
            }
            .shortcut(for: .newTab, store: keyBindings)

            Button(L10n.string("New Home Tab")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.newHomeTab)
            }
            .shortcut(for: .newHomeTab, store: keyBindings)

            Button(L10n.string("New Browser Tab")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.newBrowserTab)
            }
            .shortcut(for: .newBrowserTab, store: keyBindings)
            .disabled(activeProject == nil || !browserEnabled)

            Menu(L10n.string("Custom Commands")) {
                if commandShortcuts.shortcuts.isEmpty {
                    Button(L10n.string("No Custom Commands")) {}
                        .disabled(true)
                } else {
                    ForEach(commandShortcuts.shortcuts) { shortcut in
                        Button(shortcut.localizedDisplayName) {
                            performCommandShortcut(shortcut)
                        }
                        .disabled(shortcut.trimmedCommand.isEmpty)
                    }
                }
            }

            Divider()

            Button(L10n.string("Close Tab")) {
                guard isMainWindowFocused else {
                    NSApp.keyWindow?.performClose(nil)
                    return
                }
                performShortcutAction(.closeTab)
            }
            .shortcut(for: .closeTab, store: keyBindings)

            Divider()

            Button(L10n.string("Rename Tab")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.renameTab)
            }
            .shortcut(for: .renameTab, store: keyBindings)

            Button(L10n.string("Pin/Unpin Tab")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.pinUnpinTab)
            }
            .shortcut(for: .pinUnpinTab, store: keyBindings)

            Divider()

            Button(L10n.string("Split Right")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.splitRight)
            }
            .shortcut(for: .splitRight, store: keyBindings)

            Button(L10n.string("Split Down")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.splitDown)
            }
            .shortcut(for: .splitDown, store: keyBindings)

            Button(L10n.string("Close Pane")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.closePane)
            }
            .shortcut(for: .closePane, store: keyBindings)

            Button(L10n.string("Focus Pane Left")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneLeft)
            }
            .shortcut(for: .focusPaneLeft, store: keyBindings)

            Button(L10n.string("Focus Pane Right")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneRight)
            }
            .shortcut(for: .focusPaneRight, store: keyBindings)

            Button(L10n.string("Focus Pane Up")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneUp)
            }
            .shortcut(for: .focusPaneUp, store: keyBindings)

            Button(L10n.string("Focus Pane Down")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneDown)
            }
            .shortcut(for: .focusPaneDown, store: keyBindings)

            Button(L10n.string("Cycle Next Tab (All Panes)")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.cycleNextTabAcrossPanes)
            }
            .shortcut(for: .cycleNextTabAcrossPanes, store: keyBindings)

            Button(L10n.string("Cycle Previous Tab (All Panes)")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.cyclePreviousTabAcrossPanes)
            }
            .shortcut(for: .cyclePreviousTabAcrossPanes, store: keyBindings)
        }

        CommandGroup(after: .windowList) {
            Button(L10n.string("Next Tab")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.nextTab)
            }
            .shortcut(for: .nextTab, store: keyBindings)

            Button(L10n.string("Previous Tab")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.previousTab)
            }
            .shortcut(for: .previousTab, store: keyBindings)

            Divider()

            ForEach(1 ... 9, id: \.self) { index in
                if let action = ShortcutAction.tabAction(for: index) {
                    Button(L10n.string("Tab \(index)")) {
                        guard isMainWindowFocused else { return }
                        performShortcutAction(action)
                    }
                    .shortcut(for: action, store: keyBindings)
                }
            }
        }

        CommandGroup(after: .sidebar) {
            Button(L10n.string("Toggle Sidebar")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleSidebar)
            }
            .shortcut(for: .toggleSidebar, store: keyBindings)

            Button(L10n.string("Toggle App Layout")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleAppLayout)
            }
            .shortcut(for: .toggleAppLayout, store: keyBindings)

            Button(L10n.string("Toggle Composer")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleRichInput)
            }
            .shortcut(for: .toggleRichInput, store: keyBindings)

            Button(L10n.string("Composer Voice")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleComposerVoice)
            }
            .shortcut(for: .toggleComposerVoice, store: keyBindings)

            Button(L10n.string("Toggle Full Screen")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleFullScreen)
            }
            .shortcut(for: .toggleFullScreen, store: keyBindings)

            Divider()

            Button(L10n.string("Next Project")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.nextProject)
            }
            .shortcut(for: .nextProject, store: keyBindings)

            Button(L10n.string("Previous Project")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.previousProject)
            }
            .shortcut(for: .previousProject, store: keyBindings)

            Divider()

            ForEach(1 ... 9, id: \.self) { index in
                if let action = ShortcutAction.projectAction(for: index) {
                    Button(L10n.string("Project \(index)")) {
                        guard isMainWindowFocused else { return }
                        performShortcutAction(action)
                    }
                    .shortcut(for: action, store: keyBindings)
                }
            }

            Divider()

            Button(L10n.string("Theme Picker")) {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleThemePicker)
            }
            .shortcut(for: .toggleThemePicker, store: keyBindings)
        }

        CommandGroup(replacing: .help) {
            Button(L10n.string("Documentation")) {
                HelpLinks.openDocs()
            }

            Button(L10n.string("GitHub Repository")) {
                HelpLinks.openRepo()
            }

            Button(L10n.string("Mobile App Repository")) {
                HelpLinks.openMobileRepo()
            }

            Button(L10n.string("Discord")) {
                HelpLinks.openDiscord()
            }

            Divider()

            Button(L10n.string("What's New?")) {
                NotificationCenter.default.post(name: .openWhatsNewModal, object: nil)
            }

            Button(L10n.string("Report an Issue...")) {
                HelpLinks.openIssues()
            }
        }
    }
}
