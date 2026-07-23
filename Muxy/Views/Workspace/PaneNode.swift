import SwiftUI

struct PaneNode: View {
    let node: VisiblePaneNode
    let topLevelGroupID: UUID
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    let projectID: UUID
    let onSelectPane: (UUID, UUID) -> Void
    let onForceCloseTab: (UUID, UUID) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void

    var body: some View {
        Group {
            switch node {
            case let .pane(area, tab):
                TabAreaView(
                    area: area,
                    tab: tab,
                    topLevelGroupID: topLevelGroupID,
                    isFocused: focusedAreaID == area.id,
                    isActiveProject: isActiveProject,
                    projectID: projectID,
                    onFocus: { onSelectPane(area.id, tab.id) },
                    onForceCloseTab: { onForceCloseTab(area.id, tab.id) },
                    onDropAction: onDropAction
                )
            case let .split(branch, first, second):
                SplitContainer(
                    branch: branch,
                    first: first,
                    second: second,
                    topLevelGroupID: topLevelGroupID,
                    focusedAreaID: focusedAreaID,
                    isActiveProject: isActiveProject,
                    projectID: projectID,
                    onSelectPane: onSelectPane,
                    onForceCloseTab: onForceCloseTab,
                    onDropAction: onDropAction
                )
            }
        }
        .id(node.id)
    }
}
