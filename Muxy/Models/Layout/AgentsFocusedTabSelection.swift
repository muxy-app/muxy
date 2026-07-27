import Foundation

@MainActor
enum AgentsFocusedTabSelection {
    struct Location: Identifiable {
        let area: TabArea
        let tab: TerminalTab

        var id: UUID { tab.id }
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
                childrenByParent[parentTabID, default: []].append(Location(area: area, tab: tab))
            }
        }
        return topLevelTabs.flatMap { topLevel in
            let locations = [Location(area: topLevel.area, tab: topLevel.tab)]
                + childrenByParent[topLevel.tab.id, default: []]
            return locations.filter { location in
                guard let paneID = location.tab.content.pane?.id else { return false }
                return providerID(paneID) != nil
            }
        }
    }
}
