import SwiftUI

struct MuxyCommands: Commands {
    let appState: AppState
    let config: MuxyConfig
    let ghostty: GhosttyService

    var body: some Commands {
        CommandGroup(after: .appInfo) {
            Button("Open Configuration...") {
                NSWorkspace.shared.open(
                    [config.ghosttyConfigURL],
                    withApplicationAt: URL(fileURLWithPath: "/System/Applications/TextEdit.app"),
                    configuration: NSWorkspace.OpenConfiguration()
                )
            }
            .keyboardShortcut(",", modifiers: .command)

            Button("Reload Configuration") {
                ghostty.reloadConfig()
            }
            .keyboardShortcut("r", modifiers: [.command, .shift])
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

        CommandGroup(after: .newItem) {
            Button("New Tab") {
                guard let projectID = appState.activeProjectID else { return }
                appState.createTab(projectID: projectID)
            }
            .keyboardShortcut("t", modifiers: .command)

            Button("Close Tab") {
                guard let projectID = appState.activeProjectID,
                      let area = appState.focusedArea(for: projectID),
                      let tabID = area.activeTabID else { return }
                appState.closeTab(tabID, projectID: projectID)
            }
            .keyboardShortcut("w", modifiers: .command)

            Divider()

            Button("Rename Tab") {
                NotificationCenter.default.post(name: .renameActiveTab, object: nil)
            }
            .keyboardShortcut("t", modifiers: [.command, .shift])

            Button("Pin/Unpin Tab") {
                guard let projectID = appState.activeProjectID else { return }
                appState.togglePinActiveTab(projectID: projectID)
            }
            .keyboardShortcut("p", modifiers: [.command, .shift])

            Divider()

            Button("Split Right") {
                guard let projectID = appState.activeProjectID else { return }
                appState.splitFocusedArea(direction: .horizontal, projectID: projectID)
            }
            .keyboardShortcut("d", modifiers: .command)

            Button("Split Down") {
                guard let projectID = appState.activeProjectID else { return }
                appState.splitFocusedArea(direction: .vertical, projectID: projectID)
            }
            .keyboardShortcut("d", modifiers: [.command, .shift])

            Button("Close Pane") {
                guard let projectID = appState.activeProjectID,
                      let areaID = appState.focusedAreaID[projectID] else { return }
                appState.closeArea(areaID, projectID: projectID)
            }
            .keyboardShortcut("w", modifiers: [.command, .shift])
        }

        CommandGroup(after: .windowList) {
            ForEach(1...9, id: \.self) { index in
                Button("Tab \(index)") {
                    guard let projectID = appState.activeProjectID else { return }
                    appState.selectTabByIndex(index - 1, projectID: projectID)
                }
                .keyboardShortcut(KeyEquivalent(Character("\(index)")), modifiers: .command)
            }
        }

        CommandGroup(after: .sidebar) {
            Button(appState.sidebarVisible ? "Hide Sidebar" : "Show Sidebar") {
                withAnimation(.easeInOut(duration: 0.2)) {
                    appState.sidebarVisible.toggle()
                }
            }
            .keyboardShortcut("b", modifiers: .command)
        }

        CommandGroup(after: .toolbar) {
            Button("Next Pane") {
                guard let projectID = appState.activeProjectID else { return }
                appState.focusNextArea(projectID: projectID)
            }
            .keyboardShortcut("]", modifiers: .command)

            Button("Previous Pane") {
                guard let projectID = appState.activeProjectID else { return }
                appState.focusPreviousArea(projectID: projectID)
            }
            .keyboardShortcut("[", modifiers: .command)
        }
    }
}
