import SwiftUI

struct TabAreaView: View {
    let area: TabArea
    let tab: TerminalTab
    let topLevelGroupID: UUID
    let isFocused: Bool
    let isActiveProject: Bool
    let projectID: UUID
    let onFocus: () -> Void
    let onForceCloseTab: () -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @Environment(AppState.self) private var appState
    @Environment(\.activeWorktreeKey) private var worktreeKey
    @State private var isExternalDragHovering = false
    @State private var externalDragHideTask: Task<Void, any Error>?
    @State private var isCommandDragging = false

    private static let externalDragHideDebounce: Duration = .milliseconds(80)

    private var ownsActivePaneDrag: Bool {
        guard let drag = dragCoordinator.activeDrag,
              !drag.isTopLevel,
              let worktreeKey,
              let root = appState.workspaceRoots[worktreeKey],
              let draggedTab = root.locateTab(id: drag.tabID)?.tab
        else { return false }
        return (draggedTab.parentTabID ?? draggedTab.id) == (tab.parentTabID ?? tab.id)
    }

    var body: some View {
        TabContentView(
            tab: tab,
            area: area,
            focused: isFocused && isActiveProject,
            visible: isActiveProject,
            areaID: area.id,
            topLevelGroupID: topLevelGroupID,
            onFocus: onFocus,
            onProcessExit: onForceCloseTab,
            onSplitRequest: { direction, position in
                appState.dispatch(.splitArea(.init(
                    projectID: projectID,
                    areaID: area.id,
                    direction: direction,
                    position: position
                )))
            }
        )
        .overlay {
            if ownsActivePaneDrag,
               dragCoordinator.hoveredAreaID == area.id,
               let zone = dragCoordinator.hoveredZone
            {
                DropZoneHighlight(zone: zone)
            }
        }
        .simultaneousGesture(
            DragGesture(minimumDistance: 4, coordinateSpace: .named(DragCoordinateSpace.mainWindow))
                .onChanged { value in
                    handleCommandDragChanged(value)
                }
                .onEnded { _ in
                    handleCommandDragEnded()
                }
        )
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
            if isCommandDragging {
                dragCoordinator.cancelDrag()
                isCommandDragging = false
            }
        }
        .background {
            if ownsActivePaneDrag {
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
            if let browserState = tab.content.browserState {
                browserState.activateFind()
                return
            }
            guard let pane = tab.content.pane else { return }
            TerminalViewRegistry.shared.existingView(for: pane.id)?.startSearch()
        }
    }

    private func handleCommandDragChanged(_ value: DragGesture.Value) {
        if !isCommandDragging {
            guard NSEvent.modifierFlags.contains(.command) else { return }
            isCommandDragging = true
            onFocus()
            dragCoordinator.beginDrag(tabID: tab.id, sourceAreaID: area.id, projectID: projectID)
        }
        dragCoordinator.updatePosition(value.location)
    }

    private func handleCommandDragEnded() {
        guard isCommandDragging else { return }
        isCommandDragging = false
        guard let result = dragCoordinator.endDrag() else { return }
        onDropAction(result)
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
    let topLevelGroupID: UUID
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
                topLevelGroupID: topLevelGroupID,
                onFocus: onFocus,
                onProcessExit: onProcessExit,
                onSplitRequest: onSplitRequest
            )
        case let .extensionWebView(extensionState):
            ExtensionWebViewPane(state: extensionState, focused: focused, onFocus: onFocus)
        case let .browser(browserState):
            if browserEnabled {
                BrowserPane(
                    state: browserState,
                    focused: focused,
                    topLevelGroupID: topLevelGroupID,
                    onFocus: onFocus
                )
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
