import Foundation

@MainActor
@Observable
final class TabArea: Identifiable {
    let id: UUID
    let projectPath: String
    var tabs: [TerminalTab] = []
    var activeTabID: UUID?
    private var tabHistory: [UUID] = []

    init(projectPath: String) {
        id = UUID()
        self.projectPath = projectPath
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: projectPath))
        tabs.append(tab)
        activeTabID = tab.id
    }

    init(projectPath: String, existingTab tab: TerminalTab) {
        id = UUID()
        self.projectPath = projectPath
        tabs.append(tab)
        activeTabID = tab.id
    }

    init(restoring snapshot: TabAreaSnapshot) {
        id = snapshot.id
        projectPath = snapshot.projectPath
        tabs = snapshot.tabs.map { TerminalTab(restoring: $0) }
        if let index = snapshot.activeTabIndex, index >= 0, index < tabs.count {
            activeTabID = tabs[index].id
        } else {
            activeTabID = tabs.first?.id
        }
    }

    func snapshot() -> TabAreaSnapshot {
        let persistedTabs = tabs.filter { $0.kind != .diffViewer }
        let activeIndex = persistedTabs.firstIndex(where: { $0.id == activeTabID })
        return TabAreaSnapshot(
            id: id,
            projectPath: projectPath,
            tabs: persistedTabs.map { $0.snapshot() },
            activeTabIndex: activeIndex
        )
    }

    var activeTab: TerminalTab? {
        guard let activeTabID else { return nil }
        return tabs.first { $0.id == activeTabID }
    }

    private var firstUnpinnedIndex: Int {
        tabs.firstIndex(where: { !$0.isPinned }) ?? tabs.count
    }

    func createTab() {
        insertTab(TerminalTab(pane: TerminalPaneState(projectPath: projectPath)))
    }

    func createTab(inDirectory directory: String) {
        insertTab(TerminalTab(pane: TerminalPaneState(projectPath: directory)))
    }

    func createCommandTab(name: String, command: String) {
        let trimmedCommand = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedCommand.isEmpty else { return }
        let title = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let pane = TerminalPaneState(
            projectPath: projectPath,
            title: title.isEmpty ? Self.commandTitle(trimmedCommand) : title,
            startupCommand: trimmedCommand,
            startupCommandInteractive: true
        )
        insertTab(TerminalTab(pane: pane))
    }

    func createVCSTab() {
        insertTab(TerminalTab(vcsState: VCSStateStore.shared.state(for: projectPath)))
    }

    func createEditorTab(filePath: String, suppressInitialFocus: Bool = false) {
        if let existing = tabs.first(where: { $0.content.editorState?.filePath == filePath }) {
            selectTab(existing.id)
            return
        }
        let editorState = EditorTabState(projectPath: projectPath, filePath: filePath)
        editorState.suppressInitialFocus = suppressInitialFocus
        insertTab(TerminalTab(editorState: editorState))
    }

    func createDiffViewerTab(vcs: VCSTabState, filePath: String, isStaged: Bool) {
        if let existing = tabs.first(where: { tab in
            guard let diff = tab.content.diffViewerState else { return false }
            return diff.filePath == filePath && diff.isStaged == isStaged
        }) {
            selectTab(existing.id)
            return
        }
        insertTab(TerminalTab(diffViewerState: DiffViewerTabState(
            vcs: vcs,
            filePath: filePath,
            isStaged: isStaged
        )))
    }

    func createExternalEditorTab(filePath: String, command: String) {
        if let existing = tabs.first(where: { $0.content.pane?.externalEditorFilePath == filePath }) {
            selectTab(existing.id)
            return
        }
        let title = "\(Self.commandTitle(command)) \(URL(fileURLWithPath: filePath).lastPathComponent)"
        let pane = TerminalPaneState(
            projectPath: projectPath,
            title: title,
            startupCommand: Self.editorLaunchCommand(command: command, filePath: filePath),
            startupCommandInteractive: true,
            externalEditorFilePath: filePath
        )
        insertTab(TerminalTab(pane: pane))
    }

    static func editorLaunchCommand(command: String, filePath: String) -> String {
        if command.contains("{file}") {
            return command.replacingOccurrences(of: "{file}", with: filePath)
        }
        return command + " " + shellEscapedPath(filePath)
    }

    private static func commandTitle(_ command: String) -> String {
        let trimmed = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let first = trimmed.split(separator: " ").first else { return "Editor" }
        return String(first)
    }

    private static func shellEscapedPath(_ path: String) -> String {
        let needsQuoting = path.contains { character in
            character.isWhitespace || "'\"\\&|;$`!()[]{}<>*?".contains(character)
        }
        guard needsQuoting else { return path }
        return "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    private func insertTab(_ tab: TerminalTab) {
        tabs.append(tab)
        if let current = activeTabID {
            tabHistory.append(current)
        }
        activeTabID = tab.id
    }

    enum InsertSide { case left, right }

    func createTabAdjacent(to tabID: UUID, side: InsertSide) {
        guard let index = tabs.firstIndex(where: { $0.id == tabID }) else { return }
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: projectPath))
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
