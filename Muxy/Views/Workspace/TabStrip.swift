import SwiftUI

struct PaneTabStrip: View {
    struct TabSnapshot: Identifiable {
        let id: UUID
        let title: String
        let kind: TerminalTab.Kind
        let isPinned: Bool
        let hasCustomTitle: Bool
    }

    let areaID: UUID
    let tabs: [TabSnapshot]
    let activeTabID: UUID?
    let isFocused: Bool
    var isWindowTitleBar: Bool = false
    var showVCSButton = true
    let projectID: UUID
    let onSelectTab: (UUID) -> Void
    let onCreateTab: () -> Void
    let onCreateVCSTab: () -> Void
    let onCloseTab: (UUID) -> Void
    let onSplit: (SplitDirection) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void
    let onCreateTabAdjacent: (UUID, TabArea.InsertSide) -> Void
    let onTogglePin: (UUID) -> Void
    let onSetCustomTitle: (UUID, String?) -> Void
    let onReorderTab: (IndexSet, Int) -> Void
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @State private var dragState = TabDragState()

    static func snapshots(from tabs: [TerminalTab]) -> [TabSnapshot] {
        tabs.map { tab in
            TabSnapshot(
                id: tab.id,
                title: tab.title,
                kind: tab.kind,
                isPinned: tab.isPinned,
                hasCustomTitle: tab.customTitle != nil
            )
        }
    }

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(tabs.enumerated()), id: \.element.id) { index, tab in
                TabCell(
                    tab: tab,
                    active: tab.id == activeTabID,
                    paneFocused: isFocused,
                    hasUnread: NotificationStore.shared.hasUnread(tabID: tab.id),
                    isAnyDragging: dragState.draggedID != nil,
                    shortcutIndex: index < 9 ? index + 1 : nil,
                    onSelect: {
                        onSelectTab(tab.id)
                    },
                    onClose: { onCloseTab(tab.id) },
                    onCreateLeft: { onCreateTabAdjacent(tab.id, .left) },
                    onCreateRight: { onCreateTabAdjacent(tab.id, .right) },
                    onTogglePin: { onTogglePin(tab.id) },
                    onSetCustomTitle: { onSetCustomTitle(tab.id, $0) }
                )
                .background {
                    if dragState.draggedID != nil {
                        GeometryReader { geo in
                            Color.clear.preference(
                                key: TabFramePreferenceKey.self,
                                value: [tab.id: geo.frame(in: .named(DragCoordinateSpace.mainWindow))]
                            )
                        }
                    }
                }
                .gesture(
                    DragGesture(minimumDistance: 4, coordinateSpace: .named(DragCoordinateSpace.mainWindow))
                        .onChanged { value in
                            handleDragChanged(
                                tab: tab,
                                globalLocation: value.location,
                                dragStartGlobalLocation: value.startLocation
                            )
                        }
                        .onEnded { _ in
                            handleDragEnded()
                        }
                )
                .onTapGesture {
                    guard dragState.draggedID == nil else { return }
                    onSelectTab(tab.id)
                }
            }

            HStack(spacing: 0) {
                Spacer(minLength: 0)
                if isWindowTitleBar, let version = UpdateService.shared.availableUpdateVersion {
                    UpdateBadge(version: version) {
                        UpdateService.shared.checkForUpdates()
                    }
                    .padding(.trailing, 4)
                }
                IconButton(symbol: "magnifyingglass", size: 12) {
                    NotificationCenter.default.post(name: .quickOpen, object: nil)
                }
                .help(shortcutTooltip("Quick Open", for: .quickOpen))
                IconButton(symbol: "square.split.2x1") { onSplit(.horizontal) }
                    .help(shortcutTooltip("Split Right", for: .splitRight))
                IconButton(symbol: "square.split.1x2") { onSplit(.vertical) }
                    .help(shortcutTooltip("Split Down", for: .splitDown))
                IconButton(symbol: "plus") { onCreateTab() }
                    .help(shortcutTooltip("New Tab", for: .newTab))
                if showVCSButton {
                    FileDiffIconButton(action: onCreateVCSTab)
                        .help(shortcutTooltip("Source Control", for: .openVCSTab))
                }
            }
            .padding(.trailing, 4)
            .background(WindowDragRepresentable(alwaysEnabled: isWindowTitleBar))
        }
        .frame(height: 32)
        .onPreferenceChange(TabFramePreferenceKey.self) { frames in
            guard dragState.draggedID != nil else { return }
            dragState.frames = frames
        }
    }

    private func shortcutTooltip(_ name: String, for action: ShortcutAction) -> String {
        "\(name) (\(KeyBindingStore.shared.combo(for: action).displayString))"
    }

    private func handleDragChanged(
        tab: TabSnapshot,
        globalLocation: CGPoint,
        dragStartGlobalLocation: CGPoint
    ) {
        if dragState.draggedID == nil {
            dragState.draggedID = tab.id
            dragState.lastReorderTargetID = nil
        }

        if dragState.isInSplitMode {
            dragCoordinator.updatePosition(globalLocation)
            return
        }

        let verticalEscape = abs(globalLocation.y - dragStartGlobalLocation.y) > 24

        if verticalEscape, !tab.isPinned {
            dragState.isInSplitMode = true
            dragCoordinator.beginDrag(tabID: tab.id, sourceAreaID: areaID, projectID: projectID)
            dragCoordinator.updatePosition(globalLocation)
            return
        }

        reorderIfNeeded(at: globalLocation)
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

            guard let sourceIndex = tabs.firstIndex(where: { $0.id == draggedID }),
                  let destIndex = tabs.firstIndex(where: { $0.id == id })
            else { return }

            dragState.lastReorderTargetID = id
            let offset = destIndex > sourceIndex ? destIndex + 1 : destIndex
            withAnimation(.easeInOut(duration: 0.15)) {
                onReorderTab(IndexSet(integer: sourceIndex), offset)
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
    var lastReorderTargetID: UUID?
}

private typealias TabFramePreferenceKey = UUIDFramePreferenceKey<TabFrameTag>

private struct TabCell: View {
    let tab: PaneTabStrip.TabSnapshot
    let active: Bool
    let paneFocused: Bool
    var hasUnread: Bool = false
    var isAnyDragging: Bool = false
    var shortcutIndex: Int?
    let onSelect: () -> Void
    let onClose: () -> Void
    let onCreateLeft: () -> Void
    let onCreateRight: () -> Void
    let onTogglePin: () -> Void
    let onSetCustomTitle: (String?) -> Void
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
                    .overlay(alignment: .topTrailing) {
                        if hasUnread, !active {
                            Circle()
                                .fill(MuxyTheme.accent)
                                .frame(width: 6, height: 6)
                                .offset(x: 3, y: -3)
                        }
                    }

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
                if tab.hasCustomTitle {
                    Button("Reset Title") { onSetCustomTitle(nil) }
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
        onSetCustomTitle(trimmed.isEmpty ? nil : trimmed)
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
        } else if tab.kind == .editor {
            Image(systemName: "pencil.line")
                .font(.system(size: 12, weight: .semibold))
        } else {
            Image(systemName: "terminal")
                .font(.system(size: 12, weight: .semibold))
        }
    }
}
