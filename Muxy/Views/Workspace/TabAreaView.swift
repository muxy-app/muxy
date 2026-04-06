import SwiftUI

struct TabAreaView: View {
    let area: TabArea
    let isFocused: Bool
    let isActiveProject: Bool
    let showTabStrip: Bool
    let showVCSButton: Bool
    let projectID: UUID
    let onFocus: () -> Void
    let onSelectTab: (UUID) -> Void
    let onCreateTab: () -> Void
    let onCreateVCSTab: () -> Void
    let onCloseTab: (UUID) -> Void
    let onSplit: (SplitDirection) -> Void
    let onClose: () -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void
    @Environment(TabDragCoordinator.self) private var dragCoordinator

    var body: some View {
        VStack(spacing: 0) {
            if showTabStrip {
                PaneTabStrip(
                    area: area,
                    isFocused: isFocused,
                    showVCSButton: showVCSButton,
                    projectID: projectID,
                    onFocus: onFocus,
                    onSelectTab: onSelectTab,
                    onCreateTab: onCreateTab,
                    onCreateVCSTab: onCreateVCSTab,
                    onCloseTab: onCloseTab,
                    onSplit: onSplit,
                    onClose: onClose,
                    onDropAction: onDropAction
                )
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
            }
            ZStack {
                ForEach(area.tabs) { tab in
                    TabContentView(
                        tab: tab,
                        focused: tab.id == area.activeTabID && isFocused && isActiveProject,
                        onFocus: onFocus,
                        onProcessExit: { onCloseTab(tab.id) }
                    )
                    .zIndex(tab.id == area.activeTabID ? 1 : 0)
                    .opacity(tab.id == area.activeTabID ? 1 : 0)
                    .allowsHitTesting(tab.id == area.activeTabID)
                }
            }
            .overlay {
                if dragCoordinator.activeDrag != nil, dragCoordinator.hoveredAreaID == area.id,
                   let zone = dragCoordinator.hoveredZone
                {
                    DropZoneHighlight(zone: zone)
                }
            }
        }
        .background(GeometryReader { geo in
            Color.clear.preference(
                key: AreaFramePreferenceKey.self,
                value: [area.id: geo.frame(in: .named(DragCoordinateSpace.mainWindow))]
            )
        })
        .onReceive(NotificationCenter.default.publisher(for: .findInTerminal)) { _ in
            guard isFocused, isActiveProject else { return }
            guard let tabID = area.activeTabID,
                  let tab = area.tabs.first(where: { $0.id == tabID })
            else { return }
            guard let pane = tab.content.pane else { return }
            TerminalViewRegistry.shared.existingView(for: pane.id)?.startSearch()
        }
    }
}

private struct TabContentView: View {
    let tab: TerminalTab
    let focused: Bool
    let onFocus: () -> Void
    let onProcessExit: () -> Void

    var body: some View {
        switch tab.content {
        case let .terminal(pane):
            TerminalPane(
                state: pane,
                focused: focused,
                onFocus: onFocus,
                onProcessExit: onProcessExit
            )
        case let .vcs(vcsState):
            VCSTabView(state: vcsState, focused: focused, onFocus: onFocus)
        }
    }
}
