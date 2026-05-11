import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct FileTreeView: View {
    @Bindable var state: FileTreeState
    let onOpenFile: (String) -> Void
    let onOpenTerminal: (String) -> Void
    let onFileMoved: (String, String) -> Void

    @State private var commands: FileTreeCommands
    @State private var hasKeyboardFocus = false
    @State private var focusToken = 0

    init(
        state: FileTreeState,
        onOpenFile: @escaping (String) -> Void,
        onOpenTerminal: @escaping (String) -> Void,
        onFileMoved: @escaping (String, String) -> Void
    ) {
        self.state = state
        self.onOpenFile = onOpenFile
        self.onOpenTerminal = onOpenTerminal
        self.onFileMoved = onFileMoved
        _commands = State(initialValue: FileTreeCommands(
            state: state,
            openTerminal: onOpenTerminal,
            onFileMoved: onFileMoved
        ))
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            ScrollViewReader { proxy in
                ScrollView {
                    ZStack(alignment: .top) {
                        emptySpaceTarget
                        LazyVStack(alignment: .leading, spacing: 0) {
                            ForEach(state.flatVisibleRows()) { item in
                                switch item {
                                case let .entry(entry, depth):
                                    FileTreeRow(
                                        entry: entry,
                                        depth: depth,
                                        state: state,
                                        commands: commands,
                                        onOpenFile: onOpenFile,
                                        requestFocus: requestKeyboardFocus
                                    )
                                    .id(entry.absolutePath)
                                case let .pendingNew(pending, depth):
                                    FileTreeNewEntryRow(
                                        kind: pending.kind,
                                        depth: depth,
                                        commands: commands
                                    )
                                    .id(pending.token)
                                }
                            }
                        }
                        .padding(.vertical, UIMetrics.spacing2)
                    }
                    .frame(maxWidth: .infinity, minHeight: 0, alignment: .top)
                }
                .background(rootDropTarget)
                .onChange(of: state.pendingScrollTarget) { _, newValue in
                    guard let newValue else { return }
                    proxy.scrollTo(newValue, anchor: .center)
                    state.consumeScrollTarget()
                }
                .onChange(of: state.pendingRenamePath) { _, newValue in
                    if newValue == nil { requestKeyboardFocus() }
                }
                .onChange(of: state.pendingNewEntry?.token) { _, newValue in
                    if newValue == nil { requestKeyboardFocus() }
                }
            }
        }
        .background(MuxyTheme.bg)
        .background(keyCaptureLayer)
        .background(keyboardShortcuts)
        .contentShape(Rectangle())
        .task(id: state.rootPath) {
            state.loadRootIfNeeded()
            requestKeyboardFocus()
        }
        .alert(
            "Move \(commands.deleteAlertKind()) to Trash?",
            isPresented: deleteAlertBinding
        ) {
            Button("Move to Trash", role: .destructive) {
                commands.confirmPendingDelete()
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {
                commands.cancelPendingDelete()
            }
        } message: {
            Text(deleteAlertMessage)
        }
    }

    private var deleteAlertBinding: Binding<Bool> {
        Binding(
            get: { !state.pendingDeletePaths.isEmpty },
            set: { newValue in
                if !newValue, !state.pendingDeletePaths.isEmpty {
                    commands.cancelPendingDelete()
                }
            }
        )
    }

    private var deleteAlertMessage: String {
        let paths = state.pendingDeletePaths
        if paths.count == 1, let path = paths.first {
            return "“\((path as NSString).lastPathComponent)” will be moved to the Trash."
        }
        return "\(paths.count) items will be moved to the Trash."
    }

    private var header: some View {
        HStack(spacing: 0) {
            Text((state.rootPath as NSString).lastPathComponent)
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.head)
                .padding(.leading, UIMetrics.spacing5)
            Spacer(minLength: 0)
            ToolbarIconStrip {
                IconButton(
                    symbol: "arrow.clockwise",
                    color: MuxyTheme.fgMuted,
                    hoverColor: MuxyTheme.fg,
                    accessibilityLabel: "Refresh"
                ) {
                    state.refresh()
                }
                .help("Refresh")
                IconButton(
                    symbol: state.showOnlyChanges ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease.circle",
                    color: state.showOnlyChanges ? MuxyTheme.accent : MuxyTheme.fgMuted,
                    hoverColor: state.showOnlyChanges ? MuxyTheme.accent : MuxyTheme.fg,
                    accessibilityLabel: "Show Only Changes"
                ) {
                    state.showOnlyChanges.toggle()
                }
                .help(state.showOnlyChanges ? "Show All Files" : "Show Only Changed Files")
            }
        }
        .frame(height: UIMetrics.scaled(32))
        .contextMenu {
            FileTreeContextMenuContents(
                path: state.rootPath,
                isDirectory: true,
                includesTargetActions: false,
                commands: commands
            )
        }
    }

    private var emptySpaceTarget: some View {
        Color.clear
            .frame(maxWidth: .infinity)
            .containerRelativeFrame(.vertical)
            .contentShape(Rectangle())
            .onTapGesture {
                state.clearSelection()
                requestKeyboardFocus()
            }
            .contextMenu {
                FileTreeContextMenuContents(
                    path: state.rootPath,
                    isDirectory: true,
                    includesTargetActions: false,
                    commands: commands
                )
            }
    }

    private var rootDropTarget: some View {
        Color.clear
            .onDrop(
                of: [.fileURL],
                delegate: FileTreeDropDelegate(
                    destinationPath: state.rootPath,
                    state: state,
                    commands: commands
                )
            )
    }

    private var keyCaptureLayer: some View {
        FileTreeKeyCapture(
            focusToken: focusToken,
            hasFocus: $hasKeyboardFocus,
            canHandleNav: { canHandleNav },
            onArrowUp: { state.moveSelection(by: -1) },
            onArrowDown: { state.moveSelection(by: 1) },
            onArrowLeft: { state.collapseOrJumpToParent() },
            onArrowRight: { state.expandOrDescend() },
            onActivate: { state.activateSelection(open: onOpenFile) },
            onEscape: { NotificationCenter.default.post(name: .toggleFileTree, object: nil) },
            onDelete: {
                guard !state.selectedPaths.isEmpty else { return }
                commands.trash(paths: Array(state.selectedPaths))
            },
            onRename: {
                guard state.selectedPaths.count == 1, let path = state.selectedPaths.first else { return }
                commands.beginRename(path: path)
            }
        )
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private var keyboardShortcuts: some View {
        Group {
            shortcutButton(.delete, modifiers: [.command], enabled: !state.selectedPaths.isEmpty) {
                commands.trash(paths: Array(state.selectedPaths))
            }
            shortcutButton("c", modifiers: [.command], enabled: !state.selectedPaths.isEmpty) {
                commands.copyToClipboard(paths: Array(state.selectedPaths))
            }
            shortcutButton("x", modifiers: [.command], enabled: !state.selectedPaths.isEmpty) {
                commands.cutToClipboard(paths: Array(state.selectedPaths))
            }
            shortcutButton("v", modifiers: [.command]) {
                commands.paste(into: state.selectedFilePath ?? state.rootPath)
            }
        }
        .buttonStyle(.plain)
        .opacity(0)
        .frame(width: 0, height: 0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private func shortcutButton(
        _ key: KeyEquivalent,
        modifiers: EventModifiers = [],
        enabled: Bool = true,
        action: @escaping () -> Void
    ) -> some View {
        Button("", action: action)
            .keyboardShortcut(key, modifiers: modifiers)
            .disabled(!canHandleShortcuts || !enabled)
    }

    private var canHandleShortcuts: Bool {
        hasKeyboardFocus && canHandleNav
    }

    private var canHandleNav: Bool {
        guard state.pendingRenamePath == nil, state.pendingNewEntry == nil else { return false }
        return state.pendingDeletePaths.isEmpty
    }

    private func requestKeyboardFocus() {
        focusToken &+= 1
    }
}

private struct FileTreeRow: View {
    let entry: FileTreeEntry
    let depth: Int
    @Bindable var state: FileTreeState
    let commands: FileTreeCommands
    let onOpenFile: (String) -> Void
    let requestFocus: () -> Void
    @State private var hovered = false

    private var isSelected: Bool {
        state.isPathSelected(entry.absolutePath)
    }

    private var isRenaming: Bool {
        state.pendingRenamePath == entry.absolutePath
    }

    private var isDropHighlighted: Bool {
        entry.isDirectory && state.dropHighlightPath == entry.absolutePath
    }

    private var isCut: Bool {
        state.cutPaths.contains(entry.absolutePath)
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Color.clear.frame(width: CGFloat(depth) * UIMetrics.spacing6)
            icon
            if isRenaming {
                FileTreeRenameField(
                    initialName: entry.name,
                    commit: { commands.commitRename(originalPath: entry.absolutePath, newName: $0) },
                    cancel: { commands.cancelRename() }
                )
            } else {
                Text(entry.name)
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(textColor)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, UIMetrics.spacing3)
        .frame(height: UIMetrics.scaled(22))
        .opacity(rowOpacity)
        .background(rowBackground)
        .overlay(dropOverlay)
        .contentShape(Rectangle())
        .onTapGesture { handleTap() }
        .onHover { hovered = $0 }
        .contextMenu {
            FileTreeContextMenuContents(
                path: entry.absolutePath,
                isDirectory: entry.isDirectory,
                includesTargetActions: true,
                commands: commands
            )
        }
        .onDrag {
            NSItemProvider(object: URL(fileURLWithPath: entry.absolutePath) as NSURL)
        }
        .modifier(DropTargetModifier(
            entry: entry,
            state: state,
            commands: commands
        ))
    }

    private var rowOpacity: Double {
        if isCut { return 0.45 }
        return entry.isIgnored ? 0.45 : 1
    }

    private var rowBackground: Color {
        if isDropHighlighted { return MuxyTheme.accentSoft }
        if isSelected { return MuxyTheme.accentSoft }
        if hovered { return MuxyTheme.hover }
        return .clear
    }

    @ViewBuilder
    private var dropOverlay: some View {
        if isDropHighlighted {
            RoundedRectangle(cornerRadius: UIMetrics.scaled(3))
                .stroke(MuxyTheme.accent, lineWidth: 1)
                .padding(.horizontal, UIMetrics.spacing2)
        }
    }

    private var icon: some View {
        Image(systemName: iconSymbol)
            .font(.system(size: UIMetrics.fontFootnote))
            .foregroundStyle(iconColor)
            .frame(width: UIMetrics.iconMD)
    }

    private var iconSymbol: String {
        guard entry.isDirectory else { return "doc" }
        return state.isExpanded(entry) ? "folder.fill" : "folder"
    }

    private var iconColor: Color {
        if entry.isDirectory { return MuxyTheme.fgMuted }
        return statusColor ?? MuxyTheme.fgMuted
    }

    private var textColor: Color {
        if let statusColor { return statusColor }
        if entry.isDirectory, state.directoryHasChanges(entry.absolutePath) {
            return MuxyTheme.diffHunkFg
        }
        return MuxyTheme.fg
    }

    private var statusColor: Color? {
        guard let status = state.status(for: entry.absolutePath) else { return nil }
        switch status {
        case .modified,
             .renamed:
            return MuxyTheme.diffHunkFg
        case .added,
             .untracked:
            return MuxyTheme.diffAddFg
        case .conflict:
            return MuxyTheme.diffRemoveFg
        }
    }

    private func handleTap() {
        requestFocus()
        let modifiers = NSEvent.modifierFlags
        if modifiers.contains(.command) {
            state.toggleSelection(entry.absolutePath)
            return
        }
        if modifiers.contains(.shift) {
            state.extendSelection(to: entry.absolutePath)
            return
        }
        state.selectOnly(entry.absolutePath)
        if entry.isDirectory {
            state.toggle(entry)
        } else {
            onOpenFile(entry.absolutePath)
        }
    }
}

private struct DropTargetModifier: ViewModifier {
    let entry: FileTreeEntry
    let state: FileTreeState
    let commands: FileTreeCommands

    func body(content: Content) -> some View {
        if entry.isDirectory {
            content.onDrop(
                of: [.fileURL],
                delegate: FileTreeDropDelegate(
                    destinationPath: entry.absolutePath,
                    state: state,
                    commands: commands
                )
            )
        } else {
            content
        }
    }
}

private struct FileTreeNewEntryRow: View {
    let kind: FileTreeState.PendingEntryKind
    let depth: Int
    let commands: FileTreeCommands

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Color.clear.frame(width: CGFloat(depth) * UIMetrics.spacing6)
            Image(systemName: kind == .folder ? "folder" : "doc")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconMD)
            FileTreeRenameField(
                initialName: "",
                commit: { commands.commitNewEntry(name: $0) },
                cancel: { commands.cancelNewEntry() }
            )
            Spacer(minLength: 0)
        }
        .padding(.horizontal, UIMetrics.spacing3)
        .frame(height: UIMetrics.scaled(22))
    }
}

private struct FileTreeRenameField: View {
    let initialName: String
    let commit: (String) -> Void
    let cancel: () -> Void

    @State private var text: String = ""
    @FocusState private var focused: Bool
    @State private var didAppear = false
    @State private var didResolve = false

    var body: some View {
        TextField("", text: $text)
            .textFieldStyle(.plain)
            .font(.system(size: UIMetrics.fontBody))
            .foregroundStyle(MuxyTheme.fg)
            .focused($focused)
            .onAppear {
                guard !didAppear else { return }
                didAppear = true
                text = initialName
                Task { @MainActor in focused = true }
            }
            .onSubmit { resolve() }
            .onExitCommand {
                guard !didResolve else { return }
                didResolve = true
                cancel()
            }
            .onChange(of: focused) { _, isFocused in
                guard didAppear, !isFocused else { return }
                resolve()
            }
    }

    private func resolve() {
        guard !didResolve else { return }
        didResolve = true
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty || trimmed == initialName {
            cancel()
        } else {
            commit(trimmed)
        }
    }
}

private struct FileTreeContextMenuContents: View {
    let path: String
    let isDirectory: Bool
    let includesTargetActions: Bool
    let commands: FileTreeCommands

    private var targets: [String] {
        commands.effectiveTargets(primaryPath: path)
    }

    var body: some View {
        Button("New File") { commands.beginNewFile(in: path) }
        Button("New Folder") { commands.beginNewFolder(in: path) }
        if includesTargetActions {
            Divider()
            Button("Rename") { commands.beginRename(path: path) }
                .disabled(targets.count > 1)
            Button(targets.count > 1 ? "Delete \(targets.count) Items" : "Delete") {
                commands.trash(paths: targets)
            }
            Divider()
            Button(targets.count > 1 ? "Cut \(targets.count) Items" : "Cut") {
                commands.cutToClipboard(paths: targets)
            }
            Button(targets.count > 1 ? "Copy \(targets.count) Items" : "Copy") {
                commands.copyToClipboard(paths: targets)
            }
        }
        Divider()
        Button("Paste") { commands.paste(into: path) }
            .disabled(!FileClipboard.hasContents)
        if includesTargetActions {
            Divider()
            Button("Copy Path") { commands.copyAbsolutePath(path) }
            Button("Copy Relative Path") { commands.copyRelativePath(path) }
        }
        Divider()
        Button("Reveal in Finder") { commands.revealInFinder(path) }
        Button("Open in Terminal") { commands.openInTerminal(path: path) }
    }
}

private struct FileTreeDropDelegate: DropDelegate {
    let destinationPath: String
    let state: FileTreeState
    let commands: FileTreeCommands

    func validateDrop(info: DropInfo) -> Bool {
        info.hasItemsConforming(to: [.fileURL])
    }

    func dropEntered(info _: DropInfo) {
        state.dropHighlightPath = destinationPath
    }

    func dropExited(info _: DropInfo) {
        if state.dropHighlightPath == destinationPath {
            state.dropHighlightPath = nil
        }
    }

    func performDrop(info: DropInfo) -> Bool {
        state.dropHighlightPath = nil
        let providers = info.itemProviders(for: [.fileURL])
        guard !providers.isEmpty else { return false }

        let optionHeld = NSEvent.modifierFlags.contains(.option)
        let destination = destinationPath
        let commands = commands

        Task { @MainActor in
            var paths: [String] = []
            for provider in providers {
                if let url = await loadURL(from: provider) {
                    paths.append(url.path)
                }
            }
            guard !paths.isEmpty else { return }
            let sanitized = paths.filter { !FileSystemOperations.isInside(path: destination, ancestor: $0) }
            guard !sanitized.isEmpty else { return }
            commands.performDrop(sources: sanitized, destinationPath: destination, copy: optionHeld)
        }
        return true
    }

    private func loadURL(from provider: NSItemProvider) async -> URL? {
        await withCheckedContinuation { continuation in
            _ = provider.loadObject(ofClass: URL.self) { url, _ in
                continuation.resume(returning: url)
            }
        }
    }
}

private struct FileTreeKeyCapture: NSViewRepresentable {
    let focusToken: Int
    @Binding var hasFocus: Bool
    let canHandleNav: () -> Bool
    let onArrowUp: () -> Void
    let onArrowDown: () -> Void
    let onArrowLeft: () -> Void
    let onArrowRight: () -> Void
    let onActivate: () -> Void
    let onEscape: () -> Void
    let onDelete: () -> Void
    let onRename: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> FileTreeKeyCaptureView {
        let view = FileTreeKeyCaptureView()
        configure(view)
        context.coordinator.lastToken = focusToken
        view.requestFocusClaim()
        return view
    }

    func updateNSView(_ nsView: FileTreeKeyCaptureView, context: Context) {
        configure(nsView)
        if context.coordinator.lastToken != focusToken {
            context.coordinator.lastToken = focusToken
            nsView.requestFocusClaim()
        }
    }

    private func configure(_ view: FileTreeKeyCaptureView) {
        view.canHandleNav = canHandleNav
        view.onFocusChange = { focused in
            hasFocus = focused
        }
        view.onArrowUp = onArrowUp
        view.onArrowDown = onArrowDown
        view.onArrowLeft = onArrowLeft
        view.onArrowRight = onArrowRight
        view.onActivate = onActivate
        view.onEscape = onEscape
        view.onDelete = onDelete
        view.onRename = onRename
    }

    final class Coordinator {
        var lastToken: Int = .min
    }
}

private enum FileTreeKey: UInt16 {
    case arrowLeft = 123
    case arrowRight = 124
    case arrowDown = 125
    case arrowUp = 126
    case returnKey = 36
    case keypadEnter = 76
    case escape = 53
    case space = 49
    case delete = 51
    case forwardDelete = 117
    case f2 = 120
}

private final class FileTreeKeyCaptureView: NSView {
    var canHandleNav: (() -> Bool)?
    var onFocusChange: ((Bool) -> Void)?
    var onArrowUp: (() -> Void)?
    var onArrowDown: (() -> Void)?
    var onArrowLeft: (() -> Void)?
    var onArrowRight: (() -> Void)?
    var onActivate: (() -> Void)?
    var onEscape: (() -> Void)?
    var onDelete: (() -> Void)?
    var onRename: (() -> Void)?

    private var focusClaimPending = false

    override var acceptsFirstResponder: Bool { true }

    func requestFocusClaim() {
        if window != nil {
            window?.makeFirstResponder(self)
            return
        }
        focusClaimPending = true
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard focusClaimPending, let window else { return }
        focusClaimPending = false
        window.makeFirstResponder(self)
    }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        if result { onFocusChange?(true) }
        return result
    }

    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        if result { onFocusChange?(false) }
        return result
    }

    override func keyDown(with event: NSEvent) {
        guard canHandleNav?() ?? true, let key = FileTreeKey(rawValue: event.keyCode) else {
            super.keyDown(with: event)
            return
        }
        switch key {
        case .arrowUp: onArrowUp?()
        case .arrowDown: onArrowDown?()
        case .arrowLeft: onArrowLeft?()
        case .arrowRight: onArrowRight?()
        case .space: onActivate?()
        case .returnKey,
             .keypadEnter,
             .f2: onRename?()
        case .escape: onEscape?()
        case .delete,
             .forwardDelete: onDelete?()
        }
    }
}
