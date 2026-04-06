import AppKit
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
            .shortcut(for: .reloadConfig, store: keyBindings)

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

            Divider()

            Button("Find") {
                guard isMainWindowFocused else { return }
                NotificationCenter.default.post(name: .findInTerminal, object: nil)
            }
            .shortcut(for: .findInTerminal, store: keyBindings)
        }

        CommandGroup(replacing: .newItem) {
            Button("Open Project...") {
                openProject()
            }
            .shortcut(for: .openProject, store: keyBindings)

            Button("New Tab") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.createTab(projectID: projectID)
            }
            .shortcut(for: .newTab, store: keyBindings)

            Button("Source Control") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                VCSDisplayMode.current.route(
                    tab: { appState.createVCSTab(projectID: projectID) },
                    window: { NotificationCenter.default.post(name: .openVCSWindow, object: nil) },
                    attached: { NotificationCenter.default.post(name: .toggleAttachedVCS, object: nil) }
                )
            }
            .shortcut(for: .openVCSTab, store: keyBindings)

            Button("Close Tab") {
                guard isMainWindowFocused else {
                    NSApp.keyWindow?.performClose(nil)
                    return
                }
                guard let projectID = appState.activeProjectID,
                      let area = appState.focusedArea(for: projectID),
                      let tabID = area.activeTabID
                else { return }
                appState.closeTab(tabID, projectID: projectID)
            }
            .shortcut(for: .closeTab, store: keyBindings)

            Divider()

            Button("Rename Tab") {
                guard isMainWindowFocused else { return }
                NotificationCenter.default.post(name: .renameActiveTab, object: nil)
            }
            .shortcut(for: .renameTab, store: keyBindings)

            Button("Pin/Unpin Tab") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.togglePinActiveTab(projectID: projectID)
            }
            .shortcut(for: .pinUnpinTab, store: keyBindings)

            Divider()

            Button("Split Right") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.splitFocusedArea(direction: .horizontal, projectID: projectID)
            }
            .shortcut(for: .splitRight, store: keyBindings)

            Button("Split Down") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.splitFocusedArea(direction: .vertical, projectID: projectID)
            }
            .shortcut(for: .splitDown, store: keyBindings)

            Button("Close Pane") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID,
                      let areaID = appState.focusedAreaID[projectID]
                else { return }
                appState.closeArea(areaID, projectID: projectID)
            }
            .shortcut(for: .closePane, store: keyBindings)

            Button("Focus Pane Left") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.focusPaneLeft(projectID: projectID)
            }
            .shortcut(for: .focusPaneLeft, store: keyBindings)

            Button("Focus Pane Right") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.focusPaneRight(projectID: projectID)
            }
            .shortcut(for: .focusPaneRight, store: keyBindings)

            Button("Focus Pane Up") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.focusPaneUp(projectID: projectID)
            }
            .shortcut(for: .focusPaneUp, store: keyBindings)

            Button("Focus Pane Down") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.focusPaneDown(projectID: projectID)
            }
            .shortcut(for: .focusPaneDown, store: keyBindings)
        }

        CommandGroup(after: .windowList) {
            Button("Next Tab") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.selectNextTab(projectID: projectID)
            }
            .shortcut(for: .nextTab, store: keyBindings)

            Button("Previous Tab") {
                guard isMainWindowFocused else { return }
                guard let projectID = appState.activeProjectID else { return }
                appState.selectPreviousTab(projectID: projectID)
            }
            .shortcut(for: .previousTab, store: keyBindings)

            Divider()

            ForEach(1 ... 9, id: \.self) { index in
                if let action = ShortcutAction.tabAction(for: index) {
                    Button("Tab \(index)") {
                        guard isMainWindowFocused else { return }
                        guard let projectID = appState.activeProjectID else { return }
                        appState.selectTabByIndex(index - 1, projectID: projectID)
                    }
                    .shortcut(for: action, store: keyBindings)
                }
            }
        }

        CommandGroup(after: .sidebar) {
            Button("Next Project") {
                guard isMainWindowFocused else { return }
                appState.selectNextProject(projects: projectStore.projects)
            }
            .shortcut(for: .nextProject, store: keyBindings)

            Button("Previous Project") {
                guard isMainWindowFocused else { return }
                appState.selectPreviousProject(projects: projectStore.projects)
            }
            .shortcut(for: .previousProject, store: keyBindings)

            Divider()

            ForEach(1 ... 9, id: \.self) { index in
                if let action = ShortcutAction.projectAction(for: index) {
                    Button("Project \(index)") {
                        guard isMainWindowFocused else { return }
                        appState.selectProjectByIndex(index - 1, projects: projectStore.projects)
                    }
                    .shortcut(for: action, store: keyBindings)
                }
            }

            Divider()

            Button(appState.sidebarVisible ? "Hide Sidebar" : "Show Sidebar") {
                guard isMainWindowFocused else { return }
                withAnimation(.easeInOut(duration: 0.2)) {
                    appState.sidebarVisible.toggle()
                }
            }
            .shortcut(for: .toggleSidebar, store: keyBindings)

            Button("Theme Picker") {
                guard isMainWindowFocused else { return }
                NotificationCenter.default.post(name: .toggleThemePicker, object: nil)
            }
            .shortcut(for: .toggleThemePicker, store: keyBindings)
        }
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
