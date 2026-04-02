import SwiftUI

struct MuxyCommands: Commands {
    let appState: AppState
    let projectStore: ProjectStore
    let keyBindings: KeyBindingStore
    let config: MuxyConfig
    let ghostty: GhosttyService
    let updateService: UpdateService

    private var isMainWindowFocused: Bool {
        ShortcutContext.isMainWindow(NSApp.keyWindow)
    }

    var body: some Commands {
        CommandGroup(after: .appSettings) {
            Button("Open Configuration...") {
                NSWorkspace.shared.open(
                    [config.ghosttyConfigURL],
                    withApplicationAt: URL(fileURLWithPath: "/System/Applications/TextEdit.app"),
                    configuration: NSWorkspace.OpenConfiguration()
                )
            }

            Button("Reload Configuration") {
                ghostty.reloadConfig()
            }
            .keyboardShortcut(
                keyBindings.combo(for: .reloadConfig).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .reloadConfig).swiftUIModifiers
            )

            Divider()

            Button("Check for Updates...") {
                updateService.checkForUpdates()
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
        }

        CommandGroup(replacing: .newItem) {
            Button("New Project") {
                newProject()
            }
            .keyboardShortcut(
                keyBindings.combo(for: .newProject).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .newProject).swiftUIModifiers
            )

            Button("Open Project...") {
                openProject()
            }
            .keyboardShortcut(
                keyBindings.combo(for: .openProject).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .openProject).swiftUIModifiers
            )

            Button("New Tab") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.createTab(projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .newTab).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .newTab).swiftUIModifiers
            )

            Button("Close Tab") {
                guard isMainWindowFocused else {
                    NSApp.keyWindow?.performClose(nil)
                    return
                }
                guard let projectID = appState.activeProjectID,
                      let area = appState.focusedArea(for: projectID),
                      let tabID = area.activeTabID else { return }
                appState.closeTab(tabID, projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .closeTab).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .closeTab).swiftUIModifiers
            )

            Divider()

            Button("Rename Tab") {
                guard isMainWindowFocused else { return }
                NotificationCenter.default.post(name: .renameActiveTab, object: nil)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .renameTab).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .renameTab).swiftUIModifiers
            )

            Button("Pin/Unpin Tab") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.togglePinActiveTab(projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .pinUnpinTab).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .pinUnpinTab).swiftUIModifiers
            )

            Divider()

            Button("Split Right") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.splitFocusedArea(direction: .horizontal, projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .splitRight).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .splitRight).swiftUIModifiers
            )

            Button("Split Down") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.splitFocusedArea(direction: .vertical, projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .splitDown).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .splitDown).swiftUIModifiers
            )

            Button("Close Pane") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID,
                      let areaID = appState.focusedAreaID[projectID] else { return }
                appState.closeArea(areaID, projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .closePane).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .closePane).swiftUIModifiers
            )
        }

        CommandGroup(after: .windowList) {
            ForEach(1...9, id: \.self) { index in
                Button("Tab \(index)") {
                    guard isMainWindowFocused else { return }
                    guard let projectID = appState.activeProjectID else { return }
                    appState.selectTabByIndex(index - 1, projectID: projectID)
                }
                .keyboardShortcut(
                    keyBindings.combo(for: ShortcutAction.tabAction(for: index)!).swiftUIKeyEquivalent,
                    modifiers: keyBindings.combo(for: ShortcutAction.tabAction(for: index)!).swiftUIModifiers
                )
            }
        }

        CommandGroup(after: .sidebar) {
            ForEach(1...9, id: \.self) { index in
                Button("Project \(index)") {
                    guard isMainWindowFocused else { return }
                    appState.selectProjectByIndex(index - 1, projects: projectStore.projects)
                }
                .keyboardShortcut(
                    keyBindings.combo(for: ShortcutAction.projectAction(for: index)!).swiftUIKeyEquivalent,
                    modifiers: keyBindings.combo(for: ShortcutAction.projectAction(for: index)!).swiftUIModifiers
                )
            }

            Divider()

            Button(appState.sidebarVisible ? "Hide Sidebar" : "Show Sidebar") {
                guard isMainWindowFocused else { return }
                withAnimation(.easeInOut(duration: 0.2)) {
                    appState.sidebarVisible.toggle()
                }
            }
            .keyboardShortcut(
                keyBindings.combo(for: .toggleSidebar).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .toggleSidebar).swiftUIModifiers
            )

            Button("Theme Picker") {
                guard isMainWindowFocused else { return }
                NotificationCenter.default.post(name: .toggleThemePicker, object: nil)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .toggleThemePicker).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .toggleThemePicker).swiftUIModifiers
            )
        }

        CommandGroup(after: .toolbar) {
            Button("Next Pane") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.focusNextArea(projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .nextPane).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .nextPane).swiftUIModifiers
            )

            Button("Previous Pane") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.focusPreviousArea(projectID: projectID)
            }
            .keyboardShortcut(
                keyBindings.combo(for: .previousPane).swiftUIKeyEquivalent,
                modifiers: keyBindings.combo(for: .previousPane).swiftUIModifiers
            )
        }
    }

    private func newProject() {
        let url = FileManager.default.homeDirectoryForCurrentUser
        let project = Project(
            name: url.lastPathComponent,
            path: url.path(percentEncoded: false),
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        appState.selectProject(project)
    }

    private func openProject() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select a project folder"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let project = Project(
            name: url.lastPathComponent,
            path: url.path(percentEncoded: false),
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        appState.selectProject(project)
    }
}
