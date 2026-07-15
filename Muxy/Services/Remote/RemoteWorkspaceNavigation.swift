import Foundation
import MuxyShared

enum RemoteWorkspaceNavigation {
    static func areas(in node: SplitNodeDTO) -> [TabAreaDTO] {
        switch node {
        case let .tabArea(area):
            [area]
        case let .split(branch):
            areas(in: branch.first) + areas(in: branch.second)
        }
    }

    static func focusedArea(in workspace: WorkspaceDTO) -> TabAreaDTO? {
        let areas = areas(in: workspace.root)
        guard let focusedAreaID = workspace.focusedAreaID else { return areas.first }
        return areas.first(where: { $0.id == focusedAreaID }) ?? areas.first
    }

    static func tab(in area: TabAreaDTO, offset: Int) -> TabDTO? {
        guard !area.tabs.isEmpty else { return nil }
        let currentIndex = area.tabs.firstIndex(where: { $0.id == area.activeTabID }) ?? 0
        let index = (currentIndex + offset + area.tabs.count) % area.tabs.count
        return area.tabs[index]
    }

    static func tabAcrossAreas(in workspace: WorkspaceDTO, offset: Int) -> (area: TabAreaDTO, tab: TabDTO)? {
        let selections = areas(in: workspace.root).flatMap { area in
            area.tabs.map { (area: area, tab: $0) }
        }
        guard !selections.isEmpty else { return nil }
        let currentIndex = selections.firstIndex { selection in
            selection.area.id == workspace.focusedAreaID && selection.tab.id == selection.area.activeTabID
        } ?? 0
        let index = (currentIndex + offset + selections.count) % selections.count
        return selections[index]
    }
}
