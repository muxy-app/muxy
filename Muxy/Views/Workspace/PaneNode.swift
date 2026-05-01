import SwiftUI

struct PaneNode: View {
    let node: SplitNode
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    var showTabStrip = true
    var showVCSButton = true
    let projectID: UUID
    let shortcutOffsets: [UUID: Int]
    let onFocusArea: (UUID) -> Void
    let onSelectTab: (UUID, UUID) -> Void
    let onCreateTab: (UUID) -> Void
    let onCreateVCSTab: (UUID) -> Void
    let onCloseTab: (UUID, UUID) -> Void
    let onForceCloseTab: (UUID, UUID) -> Void
    let onSplit: (UUID, SplitDirection) -> Void
    let onCloseArea: (UUID) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void
    var showMaximizeButton = false
    var onToggleMaximize: ((UUID) -> Void)?

    var body: some View {
        switch node {
        case let .tabArea(area):
            TabAreaView(
                area: area,
                isFocused: focusedAreaID == area.id,
                isActiveProject: isActiveProject,
                showTabStrip: showTabStrip,
                showVCSButton: showVCSButton,
                projectID: projectID,
                shortcutIndexOffset: shortcutOffsets[area.id] ?? 0,
                onFocus: { onFocusArea(area.id) },
                onSelectTab: { tabID in onSelectTab(area.id, tabID) },
                onCreateTab: { onCreateTab(area.id) },
                onCreateVCSTab: { onCreateVCSTab(area.id) },
                onCloseTab: { tabID in onCloseTab(area.id, tabID) },
                onForceCloseTab: { tabID in onForceCloseTab(area.id, tabID) },
                onSplit: { dir in onSplit(area.id, dir) },
                onDropAction: onDropAction,
                showMaximizeButton: showMaximizeButton,
                onToggleMaximize: onToggleMaximize.map { toggle in { toggle(area.id) } }
            )
        case let .split(branch):
            SplitContainer(
                branch: branch,
                focusedAreaID: focusedAreaID,
                isActiveProject: isActiveProject,
                showVCSButton: showVCSButton,
                projectID: projectID,
                shortcutOffsets: shortcutOffsets,
                onFocusArea: onFocusArea,
                onSelectTab: onSelectTab,
                onCreateTab: onCreateTab,
                onCreateVCSTab: onCreateVCSTab,
                onCloseTab: onCloseTab,
                onForceCloseTab: onForceCloseTab,
                onSplit: onSplit,
                onCloseArea: onCloseArea,
                onDropAction: onDropAction,
                showMaximizeButton: showMaximizeButton,
                onToggleMaximize: onToggleMaximize
            )
        }
    }
}
