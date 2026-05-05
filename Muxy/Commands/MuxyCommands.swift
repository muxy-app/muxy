import AppKit
import SwiftUI

struct MuxyCommands: Commands {
    @ObservedObject private var ideService = IDEIntegrationService.shared

    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
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

    private var activeEditorState: EditorTabState? {
        guard let project = activeProject else { return nil }
        return appState.activeTab(for: project.id)?.content.editorState
    }

    private var activeEditorFilePath: String? {
        activeEditorState?.filePath
    }

    private var activeEditorCursorLine: Int? {
        activeEditorState?.cursorLine
    }

    private var activeEditorCursorColumn: Int? {
        activeEditorState?.cursorColumn
    }

    private var shortcutDispatcher: ShortcutActionDispatcher {
        ShortcutActionDispatcher(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            ghostty: ghostty
        )
    }

    private func performShortcutAction(_ action: ShortcutAction) {
        _ = shortcutDispatcher.perform(action, activeProject: activeProject) { project in
            VCSDisplayMode.current.route(
                tab: { appState.createVCSTab(projectID: project.id) },
                window: { NotificationCenter.default.post(name: .openVCSWindow, object: nil) },
                attached: { NotificationCenter.default.post(name: .toggleAttachedVCS, object: nil) }
            )
        }
    }

    private var isMarkdownPreviewActive: Bool {
        guard let state = activeEditorState, state.isMarkdownFile else { return false }
        return state.markdownViewMode == .preview || state.markdownViewMode == .split
    }

    private func adjustMarkdownPreviewZoom(by delta: CGFloat) {
        EditorSettings.shared.adjustMarkdownPreviewFontScale(by: delta)
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
        CommandGroup(after: .appSettings) {
            Button {
                NSWorkspace.shared.open(
                    [config.ghosttyConfigURL],
                    withApplicationAt: URL(fileURLWithPath: "/System/Applications/TextEdit.app"),
                    configuration: NSWorkspace.OpenConfiguration()
                )
            } label: {
                Label("Open Configuration...", systemImage: "doc.text")
            }

            Button {
                performShortcutAction(.reloadConfig)
            } label: {
                Label("Reload Configuration", systemImage: "arrow.clockwise")
            }
            .shortcut(for: .reloadConfig, store: keyBindings)

            Divider()

            Button {
                CLIAccessor.installCLI()
            } label: {
                Label("Install CLI", systemImage: "terminal")
            }

            Button {
                updateService.checkForUpdates()
            } label: {
                Label("Check for Updates...", systemImage: "arrow.triangle.2.circlepath")
            }
            .disabled(!updateService.canCheckForUpdates)
        }

        CommandGroup(replacing: .pasteboard) {
            Button("Cut") { NSApp.sendAction(#selector(NSText.cut(_:)), to: nil, from: nil) }
                .keyboardShortcut("x", modifiers: .command)
            Button("Copy") { NSApp.sendAction(#selector(NSText.copy(_:)), to: nil, from: nil) }
                .keyboardShortcut("c", modifiers: .command)
            Button("Paste") { NSApp.sendAction(#selector(NSText.paste(_:)), to: nil, from: nil) }
                .keyboardShortcut("v", modifiers: .command)
            Button("Select All") { NSApp.sendAction(#selector(NSText.selectAll(_:)), to: nil, from: nil) }
                .keyboardShortcut("a", modifiers: .command)

            Divider()

            Button("Find") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.findInTerminal)
            }
            .shortcut(for: .findInTerminal, store: keyBindings)
        }

        CommandGroup(replacing: .newItem) {
            Button("Open Project...") {
                performShortcutAction(.openProject)
            }
            .shortcut(for: .openProject, store: keyBindings)

            Menu("Open in IDE") {
                Button {
                    guard let activeProjectPath else { return }
                    _ = ideService.openProject(at: activeProjectPath, in: IDEIntegrationService.finderApplication)
                } label: {
                    HStack(spacing: 8) {
                        AppBundleIconView(appURL: IDEIntegrationService.finderAppURL, fallbackSystemName: "folder", size: 20)
                        Text("Finder")
                    }
                }

                Divider()

                if ideService.installedApps.isEmpty {
                    Button("No supported IDEs found") {}
                        .disabled(true)
                } else {
                    ForEach(ideService.installedApps) { ide in
                        Button {
                            guard let activeProjectPath else { return }
                            _ = ideService.openProject(
                                at: activeProjectPath,
                                highlightingFileAt: activeEditorFilePath,
                                line: activeEditorCursorLine,
                                column: activeEditorCursorColumn,
                                in: ide
                            )
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

            Button("New Tab") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.newTab)
            }
            .shortcut(for: .newTab, store: keyBindings)

            Menu("Custom Commands") {
                if commandShortcuts.shortcuts.isEmpty {
                    Button("No Custom Commands") {}
                        .disabled(true)
                } else {
                    ForEach(commandShortcuts.shortcuts) { shortcut in
                        Button(shortcut.displayName) {
                            performCommandShortcut(shortcut)
                        }
                        .disabled(shortcut.trimmedCommand.isEmpty)
                    }
                }
            }

            Button("Source Control") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.openVCSTab)
            }
            .shortcut(for: .openVCSTab, store: keyBindings)

            Button("Quick Open") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.quickOpen)
            }
            .shortcut(for: .quickOpen, store: keyBindings)

            Button("Find in Files") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.findInFiles)
            }
            .shortcut(for: .findInFiles, store: keyBindings)

            Button("Save") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.saveFile)
            }
            .shortcut(for: .saveFile, store: keyBindings)

            Divider()

            Button("Close Tab") {
                guard isMainWindowFocused else {
                    NSApp.keyWindow?.performClose(nil)
                    return
                }
                performShortcutAction(.closeTab)
            }
            .shortcut(for: .closeTab, store: keyBindings)

            Divider()

            Button("Rename Tab") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.renameTab)
            }
            .shortcut(for: .renameTab, store: keyBindings)

            Button("Pin/Unpin Tab") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.pinUnpinTab)
            }
            .shortcut(for: .pinUnpinTab, store: keyBindings)

            Divider()

            Button("Split Right") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.splitRight)
            }
            .shortcut(for: .splitRight, store: keyBindings)

            Button("Split Down") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.splitDown)
            }
            .shortcut(for: .splitDown, store: keyBindings)

            Button("Close Pane") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.closePane)
            }
            .shortcut(for: .closePane, store: keyBindings)

            Button("Focus Pane Left") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneLeft)
            }
            .shortcut(for: .focusPaneLeft, store: keyBindings)

            Button("Focus Pane Right") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneRight)
            }
            .shortcut(for: .focusPaneRight, store: keyBindings)

            Button("Focus Pane Up") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneUp)
            }
            .shortcut(for: .focusPaneUp, store: keyBindings)

            Button("Focus Pane Down") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.focusPaneDown)
            }
            .shortcut(for: .focusPaneDown, store: keyBindings)
        }

        CommandGroup(after: .toolbar) {
            Button("Zoom In Markdown Preview") {
                guard isMainWindowFocused, isMarkdownPreviewActive else { return }
                adjustMarkdownPreviewZoom(by: EditorSettings.markdownPreviewZoomStep)
            }
            .keyboardShortcut("=", modifiers: .command)
            .disabled(!isMarkdownPreviewActive)

            Button("Zoom Out Markdown Preview") {
                guard isMainWindowFocused, isMarkdownPreviewActive else { return }
                adjustMarkdownPreviewZoom(by: -EditorSettings.markdownPreviewZoomStep)
            }
            .keyboardShortcut("-", modifiers: .command)
            .disabled(!isMarkdownPreviewActive)

            Button("Reset Markdown Preview Zoom") {
                guard isMainWindowFocused, isMarkdownPreviewActive else { return }
                EditorSettings.shared.markdownPreviewFontScale = EditorSettings.defaultMarkdownPreviewFontScale
            }
            .keyboardShortcut("0", modifiers: .command)
            .disabled(!isMarkdownPreviewActive)
        }

        CommandGroup(after: .windowList) {
            Button("Next Tab") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.nextTab)
            }
            .shortcut(for: .nextTab, store: keyBindings)

            Button("Previous Tab") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.previousTab)
            }
            .shortcut(for: .previousTab, store: keyBindings)

            Divider()

            ForEach(1 ... 9, id: \.self) { index in
                if let action = ShortcutAction.tabAction(for: index) {
                    Button("Tab \(index)") {
                        guard isMainWindowFocused else { return }
                        performShortcutAction(action)
                    }
                    .shortcut(for: action, store: keyBindings)
                }
            }
        }

        CommandGroup(after: .sidebar) {
            Button("Toggle Sidebar") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleSidebar)
            }
            .shortcut(for: .toggleSidebar, store: keyBindings)

            Divider()

            Button("Open Switcher...") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.switchWorktree)
            }
            .shortcut(for: .switchWorktree, store: keyBindings)

            Divider()

            Button("Next Project") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.nextProject)
            }
            .shortcut(for: .nextProject, store: keyBindings)

            Button("Previous Project") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.previousProject)
            }
            .shortcut(for: .previousProject, store: keyBindings)

            Divider()

            ForEach(1 ... 9, id: \.self) { index in
                if let action = ShortcutAction.projectAction(for: index) {
                    Button("Project \(index)") {
                        guard isMainWindowFocused else { return }
                        performShortcutAction(action)
                    }
                    .shortcut(for: action, store: keyBindings)
                }
            }

            Divider()

            Button("Theme Picker") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleThemePicker)
            }
            .shortcut(for: .toggleThemePicker, store: keyBindings)

            Button("AI Usage") {
                guard isMainWindowFocused else { return }
                performShortcutAction(.toggleAIUsage)
            }
            .shortcut(for: .toggleAIUsage, store: keyBindings)
        }

        CommandGroup(replacing: .help) {
            Button("Muxy Help") {
                openHelpWindow()
            }

            Divider()

            Button("Documentation") {
                HelpLinks.openDocs()
            }

            Button("GitHub Repository") {
                HelpLinks.openRepo()
            }

            Button("Mobile App Repository") {
                HelpLinks.openMobileRepo()
            }

            Button("Discord") {
                HelpLinks.openDiscord()
            }

            Divider()

            Button("Report an Issue...") {
                HelpLinks.openIssues()
            }
        }
    }

    private func openHelpWindow() {
        NotificationCenter.default.post(name: .openHelpWindow, object: nil)
    }
}
