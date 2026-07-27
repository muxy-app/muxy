import Foundation

@MainActor
enum AgentsFocusedTabSelection {
    struct Location: Identifiable {
        let area: TabArea
        let tab: TerminalTab
        let pane: TerminalPaneState

        var id: UUID { pane.id }
    }

    static func resolve(
        root: SplitNode?,
        topLevelTabs: [(area: TabArea, tab: TerminalTab)],
        providerID: (UUID) -> String?
    ) -> [Location] {
        guard let root else { return [] }
        var childrenByParent: [UUID: [Location]] = [:]
        for area in root.allAreas() {
            for tab in area.tabs {
                guard let parentTabID = tab.parentTabID else { continue }
                childrenByParent[parentTabID, default: []].append(contentsOf: locations(area: area, tab: tab))
            }
        }
        return topLevelTabs.flatMap { topLevel in
            locations(area: topLevel.area, tab: topLevel.tab).filter { providerID($0.pane.id) != nil }
                + childrenByParent[topLevel.tab.id, default: []].filter { providerID($0.pane.id) != nil }
        }
    }

    private static func locations(area: TabArea, tab: TerminalTab) -> [Location] {
        tab.terminalPanes.map { Location(area: area, tab: tab, pane: $0) }
    }
}
