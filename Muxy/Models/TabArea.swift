import Foundation

@MainActor
@Observable
final class TabArea: Identifiable {
    let id = UUID()
    let projectPath: String
    var tabs: [TerminalTab] = []
    var activeTabID: UUID?
    private var tabHistory: [UUID] = []

    init(projectPath: String) {
        self.projectPath = projectPath
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: projectPath))
        tabs.append(tab)
        activeTabID = tab.id
    }

    var activeTab: TerminalTab? {
        guard let activeTabID else { return nil }
        return tabs.first { $0.id == activeTabID }
    }

    private var firstUnpinnedIndex: Int {
        tabs.firstIndex(where: { !$0.isPinned }) ?? tabs.count
    }

    func createTab() {
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: projectPath))
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
        guard let tab = tabs.first(where: { $0.id == tabID }), !tab.isPinned else { return nil }
        let closedPaneID = tab.pane.id
        tabs.removeAll { $0.id == tabID }
        tabHistory.removeAll { $0 == tabID }
        guard activeTabID == tabID else { return closedPaneID }
        let validIDs = Set(tabs.map(\.id))
        while let prev = tabHistory.popLast() {
            if validIDs.contains(prev) {
                activeTabID = prev
                return closedPaneID
            }
        }
        activeTabID = tabs.last?.id
        return closedPaneID
    }

    func selectTab(_ tabID: UUID) {
        if let current = activeTabID, current != tabID {
            tabHistory.append(current)
        }
        activeTabID = tabID
    }

    func selectTabByIndex(_ index: Int) {
        guard index >= 0, index < tabs.count else { return }
        selectTab(tabs[index].id)
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
