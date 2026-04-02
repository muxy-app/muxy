import SwiftUI

struct PaneNode: View {
    let node: SplitNode
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    var showTabStrip = true
    let onFocusArea: (UUID) -> Void
    let onSelectTab: (UUID, UUID) -> Void
    let onCreateTab: (UUID) -> Void
    let onCloseTab: (UUID, UUID) -> Void
    let onSplit: (UUID, SplitDirection) -> Void
    let onCloseArea: (UUID) -> Void

    var body: some View {
        switch node {
        case let .tabArea(area):
            TabAreaView(
                area: area,
                isFocused: focusedAreaID == area.id,
                isActiveProject: isActiveProject,
                showTabStrip: showTabStrip,
                onFocus: { onFocusArea(area.id) },
                onSelectTab: { tabID in onSelectTab(area.id, tabID) },
                onCreateTab: { onCreateTab(area.id) },
                onCloseTab: { tabID in onCloseTab(area.id, tabID) },
                onSplit: { dir in onSplit(area.id, dir) },
                onClose: { onCloseArea(area.id) }
            )
        case let .split(branch):
            SplitContainer(
                branch: branch,
                focusedAreaID: focusedAreaID,
                isActiveProject: isActiveProject,
                onFocusArea: onFocusArea,
                onSelectTab: onSelectTab,
                onCreateTab: onCreateTab,
                onCloseTab: onCloseTab,
                onSplit: onSplit,
                onCloseArea: onCloseArea
            )
        }
    }
}
