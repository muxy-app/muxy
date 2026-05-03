import MuxyShared
import SwiftUI

struct PaneTabStrip: View {
    struct TabSnapshot: Identifiable {
        let id: UUID
        let title: String
        let kind: TerminalTab.Kind
        let isPinned: Bool
        let hasCustomTitle: Bool
        let colorID: String?
    }

    let areaID: UUID
    let tabs: [TabSnapshot]
    let activeTabID: UUID?
    let isFocused: Bool
    var isWindowTitleBar: Bool = false
    var showVCSButton = true
    var showDevelopmentBadge = false
    var openInIDEProjectPath: String?
    var openInIDEFilePath: String?
    var openInIDECursorProvider: () -> (line: Int?, column: Int?) = { (nil, nil) }
    let projectID: UUID
    let onSelectTab: (UUID) -> Void
    let onCreateTab: () -> Void
    let onCreateVCSTab: () -> Void
    let onCloseTab: (UUID) -> Void
    let onCloseOtherTabs: (UUID) -> Void
    let onCloseTabsToLeft: (UUID) -> Void
    let onCloseTabsToRight: (UUID) -> Void
    let onSplit: (SplitDirection) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void
    let onCreateTabAdjacent: (UUID, TabArea.InsertSide) -> Void
    let onTogglePin: (UUID) -> Void
    let onSetCustomTitle: (UUID, String?) -> Void
    let onSetColorID: (UUID, String?) -> Void
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
                hasCustomTitle: tab.customTitle != nil,
                colorID: tab.colorID
            )
        }
    }

    var body: some View {
        HStack(spacing: 0) {
            GeometryReader { geo in
                ScrollView(.horizontal, showsIndicators: false) {
                    tabRow(availableWidth: geo.size.width)
                        .frame(minWidth: geo.size.width, alignment: .leading)
                        .background(WindowDragRepresentable(alwaysEnabled: isWindowTitleBar))
                }
            }
            .frame(maxWidth: .infinity)
            .frame(height: 32)

            HStack(spacing: 0) {
                if showDevelopmentBadge {
                    developmentBadge
                        .padding(.trailing, 6)
                }
                if isWindowTitleBar {
                    OpenInIDEControl(
                        projectPath: openInIDEProjectPath,
                        filePath: openInIDEFilePath,
                        cursorProvider: openInIDECursorProvider
                    )
                    LayoutPickerMenu(projectID: projectID)
                }
                if isWindowTitleBar, let version = UpdateService.shared.availableUpdateVersion {
                    UpdateBadge(version: version) {
                        UpdateService.shared.checkForUpdates()
                    }
                    .padding(.trailing, 4)
                }
                IconButton(symbol: "square.split.2x1", accessibilityLabel: "Split Right") { onSplit(.horizontal) }
                    .help(shortcutTooltip("Split Right", for: .splitRight))
                IconButton(symbol: "square.split.1x2", accessibilityLabel: "Split Down") { onSplit(.vertical) }
                    .help(shortcutTooltip("Split Down", for: .splitDown))
                IconButton(symbol: "plus", accessibilityLabel: "New Tab") { onCreateTab() }
                    .help(shortcutTooltip("New Tab", for: .newTab))
                if showVCSButton {
                    IconButton(symbol: "doc.text", size: 12, accessibilityLabel: "Quick Open") {
                        NotificationCenter.default.post(name: .quickOpen, object: nil)
                    }
                    .help(shortcutTooltip("Quick Open", for: .quickOpen))
                    FileDiffIconButton(action: onCreateVCSTab)
                        .help(shortcutTooltip("Source Control", for: .openVCSTab))
                    FileTreeIconButton {
                        NotificationCenter.default.post(name: .toggleFileTree, object: nil)
                    }
                    .help(shortcutTooltip("File Tree", for: .toggleFileTree))
                }
            }
            .padding(.leading, 8)
            .padding(.trailing, 4)
            .fixedSize(horizontal: true, vertical: false)
            .background(WindowDragRepresentable(alwaysEnabled: isWindowTitleBar))
        }
        .frame(height: 32)
        .onPreferenceChange(TabFramePreferenceKey.self) { frames in
            guard dragState.draggedID != nil else { return }
            dragState.frames = frames
        }
    }

    private func tabRow(availableWidth: CGFloat) -> some View {
        let count = max(tabs.count, 1)
        let effectiveWidth = availableWidth > 0 ? availableWidth : TabCell.maxWidth * CGFloat(count)
        let perTabIdeal = effectiveWidth / CGFloat(count)
        let perTabWidth = max(TabCell.minWidth, min(TabCell.maxWidth, perTabIdeal))

        return HStack(spacing: 0) {
            ForEach(Array(tabs.enumerated()), id: \.element.id) { index, tab in
                TabCell(
                    tab: tab,
                    active: tab.id == activeTabID,
                    paneFocused: isFocused,
                    hasUnread: NotificationStore.shared.hasUnread(tabID: tab.id),
                    isAnyDragging: dragState.draggedID != nil,
                    shortcutIndex: index < 9 ? index + 1 : nil,
                    closableOthersCount: closableOthersCount(excluding: tab.id),
                    closableLeftCount: closableCount(leftOf: index),
                    closableRightCount: closableCount(rightOf: index),
                    onSelect: { onSelectTab(tab.id) },
                    onClose: { onCloseTab(tab.id) },
                    onCloseOthers: { onCloseOtherTabs(tab.id) },
                    onCloseLeft: { onCloseTabsToLeft(tab.id) },
                    onCloseRight: { onCloseTabsToRight(tab.id) },
                    onCreateLeft: { onCreateTabAdjacent(tab.id, .left) },
                    onCreateRight: { onCreateTabAdjacent(tab.id, .right) },
                    onTogglePin: { onTogglePin(tab.id) },
                    onSetCustomTitle: { onSetCustomTitle(tab.id, $0) },
                    onSetColorID: { onSetColorID(tab.id, $0) }
                )
                .frame(width: perTabWidth)
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
                    DragGesture(minimumDistance: 0, coordinateSpace: .named(DragCoordinateSpace.mainWindow))
                        .onChanged { value in
                            handleDragChanged(
                                tab: tab,
                                globalLocation: value.location,
                                dragStartGlobalLocation: value.startLocation
                            )
                        }
                        .onEnded { value in
                            handleDragEnded(
                                tab: tab,
                                globalLocation: value.location,
                                dragStartGlobalLocation: value.startLocation
                            )
                        }
                )
            }
        }
    }

    private func closableOthersCount(excluding tabID: UUID) -> Int {
        tabs.count(where: { $0.id != tabID && !$0.isPinned })
    }

    private func closableCount(leftOf index: Int) -> Int {
        tabs.prefix(index).count(where: { !$0.isPinned })
    }

    private func closableCount(rightOf index: Int) -> Int {
        tabs.suffix(from: index + 1).count(where: { !$0.isPinned })
    }

    private func shortcutTooltip(_ name: String, for action: ShortcutAction) -> String {
        "\(name) (\(KeyBindingStore.shared.combo(for: action).displayString))"
    }

    private var developmentBadge: some View {
        DebugButton()
    }

    private static let dragActivationDistance: CGFloat = 4

    private func handleDragChanged(
        tab: TabSnapshot,
        globalLocation: CGPoint,
        dragStartGlobalLocation: CGPoint
    ) {
        if !dragState.didSelect {
            dragState.didSelect = true
            onSelectTab(tab.id)
        }

        let dx = globalLocation.x - dragStartGlobalLocation.x
        let dy = globalLocation.y - dragStartGlobalLocation.y
        let distance = (dx * dx + dy * dy).squareRoot()

        if dragState.draggedID == nil {
            guard distance >= Self.dragActivationDistance else { return }
            dragState.draggedID = tab.id
            dragState.lastReorderTargetID = nil
        }

        if dragState.isInSplitMode {
            dragCoordinator.updatePosition(globalLocation)
            return
        }

        if abs(dy) > 24, !tab.isPinned {
            dragState.isInSplitMode = true
            dragCoordinator.beginDrag(tabID: tab.id, sourceAreaID: areaID, projectID: projectID)
            dragCoordinator.updatePosition(globalLocation)
            return
        }

        reorderIfNeeded(at: globalLocation)
    }

    private func handleDragEnded(
        tab: TabSnapshot,
        globalLocation: CGPoint,
        dragStartGlobalLocation: CGPoint
    ) {
        if !dragState.didSelect {
            onSelectTab(tab.id)
        }
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
            dragState.didSelect = false
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
    var didSelect = false
}

private typealias TabFramePreferenceKey = UUIDFramePreferenceKey<TabFrameTag>

private struct TabWidthPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = max(value, nextValue())
    }
}

private struct TabCell: View {
    static let minWidth: CGFloat = 44
    static let maxWidth: CGFloat = 200
    static let titleHideThreshold: CGFloat = 80

    let tab: PaneTabStrip.TabSnapshot
    let active: Bool
    let paneFocused: Bool
    var hasUnread: Bool = false
    var isAnyDragging: Bool = false
    var shortcutIndex: Int?
    var closableOthersCount: Int = 0
    var closableLeftCount: Int = 0
    var closableRightCount: Int = 0
    let onSelect: () -> Void
    let onClose: () -> Void
    let onCloseOthers: () -> Void
    let onCloseLeft: () -> Void
    let onCloseRight: () -> Void
    let onCreateLeft: () -> Void
    let onCreateRight: () -> Void
    let onTogglePin: () -> Void
    let onSetCustomTitle: (String?) -> Void
    let onSetColorID: (String?) -> Void
    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var showColorPicker = false
    @State private var measuredWidth: CGFloat = TabCell.maxWidth
    @FocusState private var renameFieldFocused: Bool

    private var titleHidden: Bool {
        measuredWidth < Self.titleHideThreshold
    }

    private var tabColor: Color? {
        ProjectIconColor.color(for: tab.colorID)
    }

    private var tabBackground: Color {
        guard let tabColor else {
            return active ? MuxyTheme.surface : .clear
        }
        let opacity = if active { 0.18 } else if hovered { 0.08 } else { 0.04 }
        return tabColor.opacity(opacity)
    }

    private var bottomAccentColor: Color? {
        if active, paneFocused {
            return tabColor ?? MuxyTheme.accent
        }
        if let tabColor, !active {
            return tabColor
        }
        return nil
    }

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
                    .opacity(titleHidden && hovered && !tab.isPinned ? 0 : 1)
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
                        .onChange(of: renameFieldFocused) { _, focused in
                            if !focused, isRenaming { commitRename() }
                        }
                } else if !titleHidden {
                    Text(tab.title)
                        .font(.system(size: 12))
                        .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
            .padding(.leading, titleHidden ? 0 : 12)
            .padding(.trailing, titleHidden ? 0 : 28)
            .frame(maxWidth: .infinity, alignment: titleHidden ? .center : .leading)
            .frame(height: 32)
            .background {
                GeometryReader { geo in
                    Color.clear.preference(key: TabWidthPreferenceKey.self, value: geo.size.width)
                }
            }
            .onPreferenceChange(TabWidthPreferenceKey.self) { measuredWidth = $0 }
            .overlay(alignment: titleHidden ? .center : .trailing) {
                if !tab.isPinned {
                    let visible = titleHidden ? hovered : (active || hovered)
                    Image(systemName: "xmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .padding(.trailing, titleHidden ? 0 : 10)
                        .opacity(visible ? 1 : 0)
                        .onTapGesture(perform: onClose)
                        .accessibilityLabel("Close Tab")
                        .accessibilityAddTraits(.isButton)
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
                if let accentColor = bottomAccentColor {
                    Rectangle()
                        .fill(accentColor)
                        .frame(height: 2)
                        .accessibilityHidden(true)
                }
            }
            .background(tabBackground)
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
                        .accessibilityHidden(true)
                }
            }
            .overlay {
                DoubleClickView(action: startRename)
                    .accessibilityHidden(true)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(tabAccessibilityLabel)
            .accessibilityAddTraits(active ? .isSelected : [])
            .accessibilityAddTraits(.isButton)
            .contextMenu {
                Button("New Tab to the Left") { onCreateLeft() }
                Button("New Tab to the Right") { onCreateRight() }
                Divider()
                Button("Rename Tab") { startRename() }
                if tab.hasCustomTitle {
                    Button("Reset Title") { onSetCustomTitle(nil) }
                }
                Button("Set Tab Color…") { showColorPicker = true }
                if tab.colorID != nil {
                    Button("Reset Tab Color") { onSetColorID(nil) }
                }
                Divider()
                Button(tab.isPinned ? "Unpin Tab" : "Pin Tab") {
                    onTogglePin()
                }
                if !tab.isPinned {
                    Divider()
                    Button("Close Tab") { onClose() }
                    Button("Close Other Tabs") { onCloseOthers() }
                        .disabled(closableOthersCount == 0)
                    Button("Close Tabs to the Left") { onCloseLeft() }
                        .disabled(closableLeftCount == 0)
                    Button("Close Tabs to the Right") { onCloseRight() }
                        .disabled(closableRightCount == 0)
                }
            }
            .popover(isPresented: $showColorPicker, arrowEdge: .bottom) {
                ProjectIconColorPicker(title: "Tab Color", selectedID: tab.colorID) { id in
                    onSetColorID(id)
                    showColorPicker = false
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

    private var tabAccessibilityLabel: String {
        var label = tab.title
        switch tab.kind {
        case .terminal: label += ", Terminal"
        case .vcs: label += ", Source Control"
        case .editor: label += ", Editor"
        case .diffViewer: label += ", Diff Viewer"
        }
        if tab.isPinned { label += ", Pinned" }
        if hasUnread { label += ", Unread" }
        return label
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
        } else if tab.kind == .diffViewer {
            Image(systemName: "rectangle.split.2x1")
                .font(.system(size: 11, weight: .semibold))
        } else {
            Image(systemName: "terminal")
                .font(.system(size: 12, weight: .semibold))
        }
    }
}
