import SwiftUI

struct TabAreaView: View {
    let area: TabArea
    let isFocused: Bool
    let isActiveProject: Bool
    let showTabStrip: Bool
    let projectID: UUID
    let shortcutIndexOffset: Int
    let onFocus: () -> Void
    let onSelectTab: (UUID) -> Void
    let onCreateTab: () -> Void
    let onCloseTab: (UUID) -> Void
    let onForceCloseTab: (UUID) -> Void
    let onSplit: (SplitDirection) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void
    var showMaximizeButton = false
    var isMaximized = false
    var onToggleMaximize: (() -> Void)?
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @Environment(AppState.self) private var appState
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true
    @State private var isExternalDragHovering = false
    @State private var externalDragHideTask: Task<Void, any Error>?

    private static let externalDragHideDebounce: Duration = .milliseconds(80)

    private func closeTabs(_ tabIDs: [UUID]) {
        for tabID in tabIDs {
            onCloseTab(tabID)
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            if showTabStrip {
                PaneTabStrip(
                    areaID: area.id,
                    tabs: PaneTabStrip.snapshots(from: area.tabs),
                    activeTabID: area.activeTabID,
                    isFocused: isFocused,
                    projectID: projectID,
                    shortcutIndexOffset: shortcutIndexOffset,
                    onSelectTab: onSelectTab,
                    onCreateTab: onCreateTab,
                    onOpenBrowser: browserEnabled ? {
                        appState.dispatch(.createBrowserTab(
                            projectID: projectID,
                            areaID: area.id,
                            url: BrowserURL.homeURL,
                            profileID: BrowserPreferences.defaultProfileID
                        ))
                    } : nil,
                    onCloseTab: onCloseTab,
                    onCloseOtherTabs: { tabID in
                        closeTabs(area.tabs.filter { $0.id != tabID && !$0.isPinned }.map(\.id))
                    },
                    onCloseTabsToLeft: { tabID in
                        guard let index = area.tabs.firstIndex(where: { $0.id == tabID }) else { return }
                        closeTabs(area.tabs.prefix(index).filter { !$0.isPinned }.map(\.id))
                    },
                    onCloseTabsToRight: { tabID in
                        guard let index = area.tabs.firstIndex(where: { $0.id == tabID }) else { return }
                        closeTabs(area.tabs.suffix(from: index + 1).filter { !$0.isPinned }.map(\.id))
                    },
                    onSplit: onSplit,
                    onDropAction: onDropAction,
                    showMaximizeButton: showMaximizeButton,
                    isMaximized: isMaximized,
                    onToggleMaximize: onToggleMaximize,
                    onCreateTabAdjacent: { tabID, side in
                        area.createTabAdjacent(to: tabID, side: side)
                    },
                    onTogglePin: { tabID in
                        area.togglePin(tabID)
                    },
                    onSetCustomTitle: { tabID, title in
                        area.setCustomTitle(tabID, title: title)
                        appState.saveWorkspaces()
                    },
                    onSetColorID: { tabID, colorID in
                        area.setColorID(tabID, colorID: colorID)
                        appState.saveWorkspaces()
                    },
                    onReorderTab: { fromOffsets, toOffset in
                        area.reorderTab(fromOffsets: fromOffsets, toOffset: toOffset)
                    }
                )
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
            }
            ZStack {
                ForEach(area.tabs) { tab in
                    let isActive = tab.id == area.activeTabID
                    TabContentView(
                        tab: tab,
                        area: area,
                        focused: isActive && isFocused && isActiveProject,
                        visible: isActive && isActiveProject,
                        areaID: area.id,
                        onFocus: onFocus,
                        onProcessExit: { onForceCloseTab(tab.id) },
                        onSplitRequest: { direction, position in
                            appState.dispatch(.splitArea(.init(
                                projectID: projectID,
                                areaID: area.id,
                                direction: direction,
                                position: position
                            )))
                        }
                    )
                    .zIndex(isActive ? 1 : 0)
                    .opacity(isActive ? 1 : 0)
                    .allowsHitTesting(isActive)
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
        .overlay {
            if isExternalDragHovering {
                ExternalDragHoverHighlight()
            }
        }
        .animation(.easeInOut(duration: 0.15), value: isExternalDragHovering)
        .onReceive(NotificationCenter.default.publisher(for: .externalDragHoverChanged)) { note in
            handleExternalDragHover(note: note)
        }
        .onDisappear {
            externalDragHideTask?.cancel()
        }
        .background {
            if dragCoordinator.activeDrag != nil {
                GeometryReader { geo in
                    Color.clear.preference(
                        key: AreaFramePreferenceKey.self,
                        value: [area.id: geo.frame(in: .named(DragCoordinateSpace.mainWindow))]
                    )
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .findInTerminal)) { _ in
            guard isFocused, isActiveProject else { return }
            guard let tabID = area.activeTabID,
                  let tab = area.tabs.first(where: { $0.id == tabID })
            else { return }
            if let browserState = tab.content.browserState {
                browserState.activateFind()
                return
            }
            guard let pane = tab.content.pane else { return }
            TerminalViewRegistry.shared.existingView(for: pane.id)?.startSearch()
        }
    }

    private func handleExternalDragHover(note: Notification) {
        guard let hovering = note.userInfo?[ExternalDragHoverUserInfoKey.isHovering] as? Bool,
              let notedAreaID = note.userInfo?[ExternalDragHoverUserInfoKey.areaID] as? UUID,
              notedAreaID == area.id
        else { return }
        externalDragHideTask?.cancel()
        if hovering {
            isExternalDragHovering = true
            return
        }
        externalDragHideTask = Task { @MainActor in
            try await Task.sleep(for: Self.externalDragHideDebounce)
            isExternalDragHovering = false
        }
    }
}

private struct ExternalDragHoverHighlight: View {
    var body: some View {
        Rectangle()
            .fill(MuxyTheme.accent.opacity(0.15))
            .overlay(
                Rectangle()
                    .strokeBorder(MuxyTheme.accent.opacity(0.6), lineWidth: 2)
            )
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }
}

private struct TabContentView: View {
    let tab: TerminalTab
    let area: TabArea
    let focused: Bool
    let visible: Bool
    let areaID: UUID
    let onFocus: () -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true

    var body: some View {
        switch tab.content {
        case let .terminal(pane):
            TerminalPane(
                state: pane,
                focused: focused,
                visible: visible,
                areaID: areaID,
                onFocus: onFocus,
                onProcessExit: onProcessExit,
                onSplitRequest: onSplitRequest
            )
        case let .extensionWebView(extensionState):
            ExtensionWebViewPane(state: extensionState, focused: focused, onFocus: onFocus)
        case let .browser(browserState):
            if browserEnabled {
                BrowserPane(state: browserState, focused: focused, onFocus: onFocus)
            } else {
                BrowserDisabledPlaceholder()
                    .contentShape(Rectangle())
                    .onTapGesture { onFocus() }
            }
        }
    }
}

private struct BrowserDisabledPlaceholder: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "globe.badge.chevron.backward")
                .font(.system(size: 32, weight: .light))
            Text("Built-in browser is disabled")
                .font(.headline)
            Text("Enable it in Settings → Browser.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MuxyTheme.bg)
    }
}
