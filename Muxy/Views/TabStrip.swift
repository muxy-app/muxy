import SwiftUI

struct PaneTabStrip: View {
    let area: TabArea
    let isFocused: Bool
    var isWindowTitleBar: Bool = false
    let onFocus: () -> Void
    let onSelectTab: (UUID) -> Void
    let onCreateTab: () -> Void
    let onCreateVCSTab: () -> Void
    let onCloseTab: (UUID) -> Void
    let onSplit: (SplitDirection) -> Void
    let onClose: () -> Void
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
                .background(GeometryReader { geo in
                    Color.clear.preference(
                        key: TabFramePreferenceKey.self,
                        value: [tab.id: geo.frame(in: .named("tabstrip-\(area.id)"))]
                    )
                })
                .gesture(
                    DragGesture(
                        minimumDistance: 4,
                        coordinateSpace: .named("tabstrip-\(area.id)")
                    )
                    .onChanged { value in
                        if dragState.draggedID == nil {
                            dragState.draggedID = tab.id
                        }
                        reorderIfNeeded(at: value.location)
                    }
                    .onEnded { _ in
                        withAnimation(.easeInOut(duration: 0.15)) {
                            dragState.draggedID = nil
                        }
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
                FileDiffIconButton(action: onCreateVCSTab)
            }
            .padding(.trailing, 4)
            .background(WindowDragRepresentable(alwaysEnabled: isWindowTitleBar))
        }
        .frame(height: 32)
        .onPreferenceChange(TabFramePreferenceKey.self) { dragState.frames = $0 }
        .coordinateSpace(name: "tabstrip-\(area.id)")
    }

    private func reorderIfNeeded(at location: CGPoint) {
        guard let draggedID = dragState.draggedID else { return }
        for (id, frame) in dragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            guard let sourceIndex = area.tabs.firstIndex(where: { $0.id == draggedID }),
                  let destIndex = area.tabs.firstIndex(where: { $0.id == id })
            else { return }
            let offset = destIndex > sourceIndex ? destIndex + 1 : destIndex
            withAnimation(.easeInOut(duration: 0.15)) {
                area.reorderTab(fromOffsets: IndexSet(integer: sourceIndex), toOffset: offset)
            }
            return
        }
    }
}

private struct TabDragState {
    var draggedID: UUID?
    var frames: [UUID: CGRect] = [:]
}

private struct TabFramePreferenceKey: PreferenceKey {
    nonisolated(unsafe) static var defaultValue: [UUID: CGRect] = [:]
    static func reduce(value: inout [UUID: CGRect], nextValue: () -> [UUID: CGRect]) {
        value.merge(nextValue()) { $1 }
    }
}

struct WindowDragRepresentable: NSViewRepresentable {
    var alwaysEnabled: Bool = false

    func makeNSView(context: Context) -> WindowDragView {
        let view = WindowDragView()
        view.alwaysEnabled = alwaysEnabled
        return view
    }

    func updateNSView(_ nsView: WindowDragView, context: Context) {
        nsView.alwaysEnabled = alwaysEnabled
    }
}

final class WindowDragView: NSView {
    var alwaysEnabled = false

    private var isAtWindowTop: Bool {
        guard let window else { return false }
        let frameInWindow = convert(bounds, to: nil)
        guard let contentHeight = window.contentView?.bounds.height else { return false }
        return frameInWindow.maxY >= contentHeight - 1
    }

    override func mouseDown(with event: NSEvent) {
        guard alwaysEnabled || isAtWindowTop else {
            super.mouseDown(with: event)
            return
        }
        if event.clickCount == 2 {
            let action = UserDefaults.standard.string(forKey: "AppleActionOnDoubleClick") ?? "Maximize"
            switch action {
            case "Minimize":
                window?.miniaturize(nil)
            default:
                window?.zoom(nil)
            }
            return
        }
        let threshold = NSEvent.doubleClickInterval
        guard let next = window?.nextEvent(
            matching: [.leftMouseUp, .leftMouseDown, .leftMouseDragged],
            until: Date(timeIntervalSinceNow: threshold),
            inMode: .eventTracking,
            dequeue: true
        )
        else {
            window?.performDrag(with: event)
            return
        }
        if next.type == .leftMouseDragged {
            window?.performDrag(with: event)
        } else if next.type == .leftMouseDown {
            mouseDown(with: next)
        }
    }
}

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

private struct MiddleClickView: NSViewRepresentable {
    let action: () -> Void

    func makeNSView(context: Context) -> MiddleClickNSView {
        let view = MiddleClickNSView()
        view.action = action
        return view
    }

    func updateNSView(_ nsView: MiddleClickNSView, context: Context) {
        nsView.action = action
    }
}

private final class MiddleClickNSView: NSView {
    var action: (() -> Void)?

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let currentEvent = NSApp.currentEvent,
              currentEvent.type == .otherMouseDown,
              currentEvent.buttonNumber == 2
        else { return nil }
        return super.hitTest(point)
    }

    override func otherMouseDown(with event: NSEvent) {
        guard event.buttonNumber == 2 else {
            super.otherMouseDown(with: event)
            return
        }
        action?()
    }
}
