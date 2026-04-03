import SwiftUI

struct TabAreaView: View {
    let area: TabArea
    let isFocused: Bool
    let isActiveProject: Bool
    let showTabStrip: Bool
    let onFocus: () -> Void
    let onSelectTab: (UUID) -> Void
    let onCreateTab: () -> Void
    let onCloseTab: (UUID) -> Void
    let onSplit: (SplitDirection) -> Void
    let onClose: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            if showTabStrip {
                PaneTabStrip(
                    area: area,
                    isFocused: isFocused,
                    onFocus: onFocus,
                    onSelectTab: onSelectTab,
                    onCreateTab: onCreateTab,
                    onCloseTab: onCloseTab,
                    onSplit: onSplit,
                    onClose: onClose
                )
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
            }
            ZStack {
                ForEach(area.tabs) { tab in
                    let isActive = tab.id == area.activeTabID
                    TerminalPane(
                        state: tab.pane,
                        focused: isFocused && isActive && isActiveProject,
                        onFocus: onFocus,
                        onProcessExit: { onCloseTab(tab.id) }
                    )
                    .id(tab.id)
                    .opacity(isActive ? 1 : 0)
                    .allowsHitTesting(isActive)
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .findInTerminal)) { _ in
            guard isFocused, isActiveProject else { return }
            guard let tabID = area.activeTabID,
                  let tab = area.tabs.first(where: { $0.id == tabID })
            else { return }
            let paneID = tab.pane.id
            TerminalViewRegistry.shared.existingView(for: paneID)?.startSearch()
        }
    }
}
