import SwiftUI

struct PaneNode: View {
    let node: SplitNode
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    var showTabStrip = true
    let shortcutOffsets: [UUID: Int]
    let actions: WorkspaceViewActions
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
                projectID: actions.projectID,
                shortcutIndexOffset: shortcutOffsets[area.id] ?? 0,
                actions: actions,
                showMaximizeButton: showMaximizeButton,
                onToggleMaximize: onToggleMaximize.map { toggle in { toggle(area.id) } }
            )
        case let .split(branch):
            SplitContainer(
                branch: branch,
                focusedAreaID: focusedAreaID,
                isActiveProject: isActiveProject,
                shortcutOffsets: shortcutOffsets,
                actions: actions,
                showMaximizeButton: showMaximizeButton,
                onToggleMaximize: onToggleMaximize
            )
        }
    }
}
