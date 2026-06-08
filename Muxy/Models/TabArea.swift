import Foundation

@MainActor
@Observable
final class TabArea: Identifiable {
    let id: UUID
    let projectPath: String
    let remoteConfig: RemoteProjectConfig?
    var tabs: [TerminalTab] = []
    var activeTabID: UUID?
    private var tabHistory: [UUID] = []

    init(projectPath: String, remoteConfig: RemoteProjectConfig? = nil) {
        id = UUID()
        self.projectPath = projectPath
        self.remoteConfig = remoteConfig
        let pane = Self.paneForProject(projectPath: projectPath, remoteConfig: remoteConfig)
        let tab = TerminalTab(pane: pane)
        tabs.append(tab)
        activeTabID = tab.id
    }

    init(projectPath: String, command: String?, remoteConfig: RemoteProjectConfig? = nil) {
        id = UUID()
        self.projectPath = projectPath
        self.remoteConfig = remoteConfig
        let effectiveCommand: String? = if let remoteConfig, let command {
            Self.sshCommandForRemote(config: remoteConfig, injectedCommand: command)
        } else {
            command.map { "(\($0)); exec \"$0\" -l" }
        }
        let pane = TerminalPaneState(
            projectPath: projectPath,
            startupCommand: effectiveCommand,
            startupCommandInteractive: effectiveCommand != nil
        )
        let tab = TerminalTab(pane: pane)
        tabs.append(tab)
        activeTabID = tab.id
    }

    init(projectPath: String, existingTab tab: TerminalTab, remoteConfig: RemoteProjectConfig? = nil) {
        id = UUID()
        self.projectPath = projectPath
        self.remoteConfig = remoteConfig
        tabs.append(tab)
        activeTabID = tab.id
    }

    init(restoring snapshot: TabAreaSnapshot, sessionsByPaneID: [UUID: TerminalSessionSnapshot] = [:]) {
        id = snapshot.id
        projectPath = snapshot.projectPath
        remoteConfig = snapshot.remoteConfig
        tabs = snapshot.tabs.map { tabSnapshot in
            TerminalTab(
                restoring: tabSnapshot,
                restoredSession: tabSnapshot.paneID.flatMap { sessionsByPaneID[$0] }
            )
        }
        if let index = snapshot.activeTabIndex, index >= 0, index < tabs.count {
            activeTabID = tabs[index].id
        } else {
            activeTabID = tabs.first?.id
        }
    }

    func snapshot() -> TabAreaSnapshot {
        let persistedTabs = tabs
        let activeIndex = persistedTabs.firstIndex(where: { $0.id == activeTabID })
        var snapshot = TabAreaSnapshot(
            id: id,
            projectPath: projectPath,
            tabs: persistedTabs.map { $0.snapshot() },
            activeTabIndex: activeIndex
        )
        snapshot.remoteConfig = remoteConfig
        return snapshot
    }

    var activeTab: TerminalTab? {
        guard let activeTabID else { return nil }
        return tabs.first { $0.id == activeTabID }
    }

    private var firstUnpinnedIndex: Int {
        tabs.firstIndex(where: { !$0.isPinned }) ?? tabs.count
    }

    func createTab() {
        let pane = Self.paneForProject(projectPath: projectPath, remoteConfig: remoteConfig)
        insertTab(TerminalTab(pane: pane))
    }

    func createTab(inDirectory directory: String) {
        insertTab(TerminalTab(pane: TerminalPaneState(projectPath: directory)))
    }

    func createCommandTab(name: String, command: String, closesOnCommandExit: Bool = true) {
        let trimmedCommand = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedCommand.isEmpty else { return }
        let title = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let effectiveCommand = Self.sshWrappedCommand(command: trimmedCommand, remoteConfig: remoteConfig)
        let pane = TerminalPaneState(
            projectPath: projectPath,
            title: title.isEmpty ? Self.commandTitle(trimmedCommand) : title,
            startupCommand: effectiveCommand,
            startupCommandInteractive: effectiveCommand != nil,
            closesOnStartupCommandExit: closesOnCommandExit
        )
        insertTab(TerminalTab(pane: pane))
    }

    func restoreClosedTerminalTab(_ snapshot: ClosedTerminalTabSnapshot) {
        let command = snapshot.commandToRestore
        let safeCommand = command.flatMap { TerminalSessionRestorePolicy.isSafeToRestore($0) ? $0 : nil }
        let pane = TerminalPaneState(
            projectPath: snapshot.projectPath,
            title: snapshot.title,
            initialWorkingDirectory: snapshot.workingDirectory,
            startupCommand: safeCommand,
            startupCommandInteractive: safeCommand != nil
        )
        let tab = TerminalTab(pane: pane)
        tab.customTitle = snapshot.customTitle
        tab.colorID = snapshot.colorID
        insertTab(tab)
    }

    func findExtensionTab(extensionID: String, tabTypeID: String) -> TerminalTab? {
        tabs.first { tab in
            guard let state = tab.content.extensionState else { return false }
            return state.extensionID == extensionID && state.tabTypeID == tabTypeID
        }
    }

    func createExtensionTab(extensionID: String, tabTypeID: String, title: String, data: ExtensionJSON?) {
        let state = ExtensionTabState(
            extensionID: extensionID,
            tabTypeID: tabTypeID,
            projectPath: projectPath,
            defaultTitle: title,
            data: data
        )
        insertTab(TerminalTab(extensionState: state))
    }

    private static func commandTitle(_ command: String) -> String {
        let trimmed = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let first = trimmed.split(separator: " ").first else { return "Editor" }
        return String(first)
    }

    private func insertTab(_ tab: TerminalTab) {
        tabs.append(tab)
        if let current = activeTabID {
            tabHistory.append(current)
        }
        activeTabID = tab.id
    }

    private static func paneForProject(projectPath: String, remoteConfig: RemoteProjectConfig?) -> TerminalPaneState {
        guard let remoteConfig else {
            return TerminalPaneState(projectPath: projectPath)
        }
        let sshCommand = sshCommandForRemote(config: remoteConfig)
        let pane = TerminalPaneState(
            projectPath: projectPath,
            startupCommand: sshCommand,
            startupCommandInteractive: true,
            closesOnStartupCommandExit: false
        )
        pane.remoteHostID = remoteConfig.hostID
        pane.sshStartTime = Date()
        if let host = RemoteHostStore.shared.find(byID: remoteConfig.hostID), host.useKeychain,
           let password = KeychainSSHHelper.getPassword(host: host.host, user: host.user) {
            pane.envVars = [
                ("MUXY_SSH_USER", host.user),
                ("MUXY_SSH_HOST", host.host),
                ("MUXY_SSH_PORT", "\(host.port)"),
                ("MUXY_SSH_REMOTE_PATH", remoteConfig.remotePath),
                ("MUXY_SSH_PASSWORD", password),
                ("MUXY_SSH_CONTROL_PATH", host.controlPath()),
            ]
        }
        return pane
    }

    private static func askpassScriptPath() -> String {
        Bundle.appResources.resourceURL?
            .appendingPathComponent("scripts/muxy-ssh-askpass.sh")
            .path ?? ""
    }

    private static func sshWrappedCommand(command: String?, remoteConfig: RemoteProjectConfig?) -> String? {
        guard let command else { return nil }
        if let remoteConfig {
            return sshCommandForRemote(config: remoteConfig, injectedCommand: command)
        }
        return command
    }

    private static func sshCommandForRemote(config: RemoteProjectConfig, injectedCommand: String? = nil) -> String {
        let store = RemoteHostStore.shared
        guard let host = store.find(byID: config.hostID) else {
            return injectedCommand ?? "echo 'Host not found'"
        }
        var args = host.sshCommandArgs(remotePath: nil)
        if let injectedCommand {
            args.removeLast()
            args.append("\(host.user)@\(host.host)")
            args.append("-t")
            args.append("cd \(ShellEscaper.escape(config.remotePath)); \(injectedCommand)")
        } else {
            args.removeLast()
            args.append("\(host.user)@\(host.host)")
            args.append("-t")
            args.append("cd \(ShellEscaper.escape(config.remotePath)); exec $SHELL -l")
        }
        if host.useKeychain, KeychainSSHHelper.getPassword(host: host.host, user: host.user) != nil {
            return "/usr/bin/expect \(askpassScriptPath())"
        }
        let command = args.map { arg in
            if arg.contains(" ") || arg.contains(";") {
                return "'\(arg.replacingOccurrences(of: "'", with: "'\\''"))'"
            }
            return arg
        }.joined(separator: " ")
        return command
    }

    enum InsertSide { case left, right }

    func createTabAdjacent(to tabID: UUID, side: InsertSide) {
        guard let index = tabs.firstIndex(where: { $0.id == tabID }) else { return }
        let pane = Self.paneForProject(projectPath: projectPath, remoteConfig: remoteConfig)
        let tab = TerminalTab(pane: pane)
        let desiredIndex = side == .left ? index : index + 1
        let insertIndex = max(desiredIndex, firstUnpinnedIndex)
        tabs.insert(tab, at: insertIndex)
        if let current = activeTabID {
            tabHistory.append(current)
        }
        activeTabID = tab.id
    }

    func closeTab(_ tabID: UUID) -> UUID? {
        guard let tab = removeTab(tabID) else { return nil }
        return tab.content.pane?.id
    }

    func selectTab(_ tabID: UUID) {
        guard activeTabID != tabID else { return }
        if let current = activeTabID, current != tabID {
            tabHistory.append(current)
        }
        activeTabID = tabID
    }

    func selectTabByIndex(_ index: Int) {
        guard index >= 0, index < tabs.count else { return }
        selectTab(tabs[index].id)
    }

    func selectNextTab() {
        guard tabs.count > 1, let activeTabID,
              let index = tabs.firstIndex(where: { $0.id == activeTabID })
        else { return }
        let next = (index + 1) % tabs.count
        selectTab(tabs[next].id)
    }

    func selectPreviousTab() {
        guard tabs.count > 1, let activeTabID,
              let index = tabs.firstIndex(where: { $0.id == activeTabID })
        else { return }
        let previous = (index - 1 + tabs.count) % tabs.count
        selectTab(tabs[previous].id)
    }

    func reorderTab(fromOffsets source: IndexSet, toOffset destination: Int) {
        tabs.move(fromOffsets: source, toOffset: destination)
    }

    func removeTab(_ tabID: UUID) -> TerminalTab? {
        guard let index = tabs.firstIndex(where: { $0.id == tabID }) else { return nil }
        let tab = tabs[index]
        guard !tab.isPinned else { return nil }
        tabs.remove(at: index)
        tabHistory.removeAll { $0 == tabID }
        guard activeTabID == tabID else { return tab }
        let validIDs = Set(tabs.map(\.id))
        while let prev = tabHistory.popLast() {
            if validIDs.contains(prev) {
                activeTabID = prev
                return tab
            }
        }
        activeTabID = tabs.last?.id
        return tab
    }

    func insertExistingTab(_ tab: TerminalTab) {
        let insertIndex = tab.isPinned ? firstUnpinnedIndex : tabs.count
        tabs.insert(tab, at: insertIndex)
        if let current = activeTabID {
            tabHistory.append(current)
        }
        activeTabID = tab.id
    }

    func setCustomTitle(_ tabID: UUID, title: String?) {
        guard let tab = tabs.first(where: { $0.id == tabID }) else { return }
        tab.customTitle = title
    }

    func setColorID(_ tabID: UUID, colorID: String?) {
        guard let tab = tabs.first(where: { $0.id == tabID }) else { return }
        tab.colorID = colorID
    }

    func togglePin(_ tabID: UUID) {
        guard let index = tabs.firstIndex(where: { $0.id == tabID }) else { return }
        let tab = tabs[index]
        tab.isPinned.toggle()
        tabs.remove(at: index)
        if tab.isPinned {
            tabs.insert(tab, at: firstUnpinnedIndex)
        } else {
            let insertIndex = max(firstUnpinnedIndex, 0)
            tabs.insert(tab, at: insertIndex)
        }
    }
}
