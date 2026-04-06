import SwiftUI

struct PaneTabStrip: View {
    let area: TabArea
    let isFocused: Bool
    var isWindowTitleBar: Bool = false
    var showVCSButton = true
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
    @State private var dragState = TabDragState()

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(area.tabs.enumerated()), id: \.element.id) { index, tab in
                TabCell(
                    tab: tab,
                    active: tab.id == area.activeTabID,
                    paneFocused: isFocused,
                    isAnyDragging: dragState.draggedID != nil,
                    shortcutIndex: index < 9 ? index + 1 : nil,
                    onSelect: {
                        onFocus()
                        onSelectTab(tab.id)
                    },
                    onClose: { onCloseTab(tab.id) },
                    onCreateLeft: { area.createTabAdjacent(to: tab.id, side: .left) },
                    onCreateRight: { area.createTabAdjacent(to: tab.id, side: .right) },
                    onTogglePin: { area.togglePin(tab.id) }
                )
                .background {
                    if dragState.draggedID != nil {
                        GeometryReader { geo in
                            Color.clear.preference(
                                key: TabFramePreferenceKey.self,
                                value: [tab.id: geo.frame(in: .named("tabstrip-\(area.id)"))]
                            )
                        }
                    }
                }
                .gesture(
                    DragGesture(minimumDistance: 4, coordinateSpace: .named(DragCoordinateSpace.mainWindow))
                        .onChanged { value in
                            handleDragChanged(tab: tab, globalLocation: value.location)
                        }
                        .onEnded { _ in
                            handleDragEnded()
                        }
                )
                .onTapGesture {
                    guard dragState.draggedID == nil else { return }
                    onFocus()
                    onSelectTab(tab.id)
                }
            }

            HStack(spacing: 0) {
                Spacer(minLength: 0)
                IconButton(symbol: "square.split.2x1") { onSplit(.horizontal) }
                IconButton(symbol: "square.split.1x2") { onSplit(.vertical) }
                IconButton(symbol: "plus") { onCreateTab() }
                if showVCSButton {
                    FileDiffIconButton(action: onCreateVCSTab)
                }
            }
            .padding(.trailing, 4)
            .background(WindowDragRepresentable(alwaysEnabled: isWindowTitleBar))
        }
        .frame(height: 32)
        .background(GeometryReader { geo in
            Color.clear
                .onAppear { dragState.stripFrameGlobal = geo.frame(in: .named(DragCoordinateSpace.mainWindow)) }
                .onChange(of: geo.frame(in: .named(DragCoordinateSpace.mainWindow))) { _, newFrame in
                    dragState.stripFrameGlobal = newFrame
                }
        })
        .onPreferenceChange(TabFramePreferenceKey.self) { frames in
            guard dragState.draggedID != nil else { return }
            dragState.frames = frames
        }
        .coordinateSpace(name: "tabstrip-\(area.id)")
    }

    private func handleDragChanged(tab: TerminalTab, globalLocation: CGPoint) {
        if dragState.draggedID == nil {
            dragState.draggedID = tab.id
            dragState.lastReorderTargetID = nil
        }

        if dragState.isInSplitMode {
            dragCoordinator.updatePosition(globalLocation)
            return
        }

        let stripFrame = dragState.stripFrameGlobal
        let verticalEscape = globalLocation.y < stripFrame.minY - 20
            || globalLocation.y > stripFrame.maxY + 20

        if verticalEscape, !tab.isPinned {
            dragState.isInSplitMode = true
            dragCoordinator.beginDrag(tabID: tab.id, sourceAreaID: area.id, projectID: projectID)
            dragCoordinator.updatePosition(globalLocation)
            return
        }

        let localX = globalLocation.x - stripFrame.minX
        let localY = globalLocation.y - stripFrame.minY
        reorderIfNeeded(at: CGPoint(x: localX, y: localY))
    }

    private func handleDragEnded() {
        if dragState.isInSplitMode {
            if let result = dragCoordinator.endDrag() {
                onDropAction(result)
            }
        }
        withAnimation(.easeInOut(duration: 0.15)) {
            dragState.draggedID = nil
            dragState.isInSplitMode = false
            dragState.frames = [:]
            dragState.lastReorderTargetID = nil
        }
    }

    private func reorderIfNeeded(at location: CGPoint) {
        guard let draggedID = dragState.draggedID else { return }
        var hoveredTargetID: UUID?

        for (id, frame) in dragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            hoveredTargetID = id
            guard dragState.lastReorderTargetID != id else { return }

            guard let sourceIndex = area.tabs.firstIndex(where: { $0.id == draggedID }),
                  let destIndex = area.tabs.firstIndex(where: { $0.id == id })
            else { return }

            dragState.lastReorderTargetID = id
            let offset = destIndex > sourceIndex ? destIndex + 1 : destIndex
            withAnimation(.easeInOut(duration: 0.15)) {
                area.reorderTab(fromOffsets: IndexSet(integer: sourceIndex), toOffset: offset)
            }
            return
        }

        if hoveredTargetID == nil {
            dragState.lastReorderTargetID = nil
        }
    }
}

private struct TabDragState {
    var draggedID: UUID?
    var frames: [UUID: CGRect] = [:]
    var isInSplitMode = false
    var stripFrameGlobal: CGRect = .zero
    var lastReorderTargetID: UUID?
}

private typealias TabFramePreferenceKey = UUIDFramePreferenceKey<TabFrameTag>

private struct TabCell: View {
    @Bindable var tab: TerminalTab
    let active: Bool
    let paneFocused: Bool
    var isAnyDragging: Bool = false
    var shortcutIndex: Int?
    let onSelect: () -> Void
    let onClose: () -> Void
    let onCreateLeft: () -> Void
    let onCreateRight: () -> Void
    let onTogglePin: () -> Void
    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @FocusState private var renameFieldFocused: Bool

    private var showBadge: Bool {
        guard let shortcutIndex,
              let action = ShortcutAction.tabAction(for: shortcutIndex)
        else { return false }
        return ModifierKeyMonitor.shared.isHolding(
            modifiers: KeyBindingStore.shared.combo(for: action).modifiers
        )
    }

    var body: some View {
        HStack(spacing: 0) {
            HStack(spacing: 6) {
                tabIconView
                    .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)

                if isRenaming {
                    TextField("", text: $renameText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fg)
                        .focused($renameFieldFocused)
                        .onSubmit { commitRename() }
                        .onExitCommand { cancelRename() }
                } else {
                    Text(tab.title)
                        .font(.system(size: 12))
                        .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
            .padding(.leading, 12)
            .padding(.trailing, 28)
            .frame(maxWidth: 200, alignment: .leading)
            .frame(height: 32)
            .overlay(alignment: .trailing) {
                if !tab.isPinned {
                    Image(systemName: "xmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .padding(.trailing, 10)
                        .opacity(active || hovered ? 1 : 0)
                        .onTapGesture(perform: onClose)
                }
            }
            .overlay {
                if showBadge, let shortcutIndex,
                   let action = ShortcutAction.tabAction(for: shortcutIndex)
                {
                    ShortcutBadge(label: KeyBindingStore.shared.combo(for: action).displayString)
                }
            }
            .overlay(alignment: .bottom) {
                if active, paneFocused {
                    Rectangle()
                        .fill(MuxyTheme.accent)
                        .frame(height: 2)
                }
            }
            .background(active ? MuxyTheme.surface : .clear)
            .contentShape(Rectangle())
            .onHover { hovering in
                guard !isAnyDragging else { return }
                hovered = hovering
            }
            .onChange(of: isAnyDragging) { _, dragging in
                if dragging { hovered = false }
            }
            .overlay {
                if !tab.isPinned {
                    MiddleClickView(action: onClose)
                }
            }
            .contextMenu {
                Button("New Tab to the Left") { onCreateLeft() }
                Button("New Tab to the Right") { onCreateRight() }
                Divider()
                Button("Rename Tab") { startRename() }
                if tab.customTitle != nil {
                    Button("Reset Title") { tab.customTitle = nil }
                }
                Divider()
                Button(tab.isPinned ? "Unpin Tab" : "Pin Tab") {
                    onTogglePin()
                }
                if !tab.isPinned {
                    Divider()
                    Button("Close Tab") { onClose() }
                }
            }

            Rectangle().fill(MuxyTheme.border).frame(width: 1)
        }
        .onReceive(NotificationCenter.default.publisher(for: .renameActiveTab)) { _ in
            guard active else { return }
            startRename()
        }
    }

    private func startRename() {
        renameText = tab.title
        isRenaming = true
        renameFieldFocused = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        tab.customTitle = trimmed.isEmpty ? nil : trimmed
        isRenaming = false
    }

    private func cancelRename() {
        isRenaming = false
    }

    @ViewBuilder
    private var tabIconView: some View {
        if tab.isPinned {
            Image(systemName: "pin.fill")
                .font(.system(size: 10, weight: .semibold))
        } else if tab.kind == .vcs {
            FileDiffIcon()
                .stroke(style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                .frame(width: 12, height: 12)
        } else {
            Image(systemName: "terminal")
                .font(.system(size: 12, weight: .semibold))
        }
    }
}
