import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct FileTreeView: View {
    @Bindable var state: FileTreeState
    let onOpenFile: (String) -> Void
    let onOpenTerminal: (String) -> Void
    let onFileMoved: (String, String) -> Void

    @State private var commands: FileTreeCommands
    @FocusState private var treeFocused: Bool

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
            ScrollView {
                ZStack(alignment: .top) {
                    emptySpaceTarget
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(state.visibleRootEntries(), id: \.absolutePath) { entry in
                            FileTreeRowGroup(
                                entry: entry,
                                depth: 0,
                                state: state,
                                commands: commands,
                                onOpenFile: onOpenFile,
                                requestFocus: { treeFocused = true }
                            )
                        }
                        if let pending = state.pendingNewEntry, pending.parentPath == normalizedRootPath {
                            FileTreeNewEntryRow(
                                kind: pending.kind,
                                depth: 0,
                                commands: commands
                            )
                            .id(pending.token)
                        }
                    }
                    .padding(.vertical, 4)
                }
                .frame(maxWidth: .infinity, minHeight: 0, alignment: .top)
            }
            .background(rootDropTarget)
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .focusable()
        .focusEffectDisabled()
        .focused($treeFocused)
        .background(keyboardShortcuts)
        .task(id: state.rootPath) {
            state.loadRootIfNeeded()
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
        HStack(spacing: 8) {
            Text((state.rootPath as NSString).lastPathComponent)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.head)
            Spacer(minLength: 0)
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
        .padding(.horizontal, 10)
        .frame(height: 32)
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
                treeFocused = true
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

    private var keyboardShortcuts: some View {
        Group {
            Button("") {
                guard state.selectedPaths.count == 1, let path = state.selectedPaths.first else { return }
                commands.beginRename(path: path)
            }
            .keyboardShortcut(.return, modifiers: [])

            Button("") {
                let paths = Array(state.selectedPaths)
                guard !paths.isEmpty else { return }
                commands.trash(paths: paths)
            }
            .keyboardShortcut(.delete, modifiers: [])

            Button("") {
                let paths = Array(state.selectedPaths)
                guard !paths.isEmpty else { return }
                commands.trash(paths: paths)
            }
            .keyboardShortcut(.delete, modifiers: [.command])

            Button("") {
                let paths = Array(state.selectedPaths)
                guard !paths.isEmpty else { return }
                commands.copyToClipboard(paths: paths)
            }
            .keyboardShortcut("c", modifiers: [.command])

            Button("") {
                let paths = Array(state.selectedPaths)
                guard !paths.isEmpty else { return }
                commands.cutToClipboard(paths: paths)
            }
            .keyboardShortcut("x", modifiers: [.command])

            Button("") {
                let target = state.selectedFilePath ?? state.rootPath
                commands.paste(into: target)
            }
            .keyboardShortcut("v", modifiers: [.command])
        }
        .buttonStyle(.plain)
        .opacity(0)
        .frame(width: 0, height: 0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private var normalizedRootPath: String {
        state.rootPath.hasSuffix("/") ? String(state.rootPath.dropLast()) : state.rootPath
    }
}

private struct FileTreeRowGroup: View {
    let entry: FileTreeEntry
    let depth: Int
    @Bindable var state: FileTreeState
    let commands: FileTreeCommands
    let onOpenFile: (String) -> Void
    let requestFocus: () -> Void

    var body: some View {
        FileTreeRow(
            entry: entry,
            depth: depth,
            state: state,
            commands: commands,
            onOpenFile: onOpenFile,
            requestFocus: requestFocus
        )
        if entry.isDirectory, state.isExpanded(entry), let children = state.visibleChildren(of: entry) {
            ForEach(children, id: \.absolutePath) { child in
                FileTreeRowGroup(
                    entry: child,
                    depth: depth + 1,
                    state: state,
                    commands: commands,
                    onOpenFile: onOpenFile,
                    requestFocus: requestFocus
                )
            }
            if let pending = state.pendingNewEntry, pending.parentPath == entry.absolutePath {
                FileTreeNewEntryRow(kind: pending.kind, depth: depth + 1, commands: commands)
                    .id(pending.token)
            }
        }
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
        HStack(spacing: 4) {
            Color.clear.frame(width: CGFloat(depth) * 12)
            icon
            if isRenaming {
                FileTreeRenameField(
                    initialName: entry.name,
                    commit: { commands.commitRename(originalPath: entry.absolutePath, newName: $0) },
                    cancel: { commands.cancelRename() }
                )
            } else {
                Text(entry.name)
                    .font(.system(size: 12))
                    .foregroundStyle(textColor)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6)
        .frame(height: 22)
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
            RoundedRectangle(cornerRadius: 3)
                .stroke(MuxyTheme.accent, lineWidth: 1)
                .padding(.horizontal, 4)
        }
    }

    private var icon: some View {
        Image(systemName: iconSymbol)
            .font(.system(size: 11))
            .foregroundStyle(iconColor)
            .frame(width: 14)
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
        case .deleted,
             .conflict:
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
        } else if state.status(for: entry.absolutePath) != .deleted {
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
        HStack(spacing: 4) {
            Color.clear.frame(width: CGFloat(depth) * 12)
            Image(systemName: kind == .folder ? "folder" : "doc")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 14)
            FileTreeRenameField(
                initialName: "",
                commit: { commands.commitNewEntry(name: $0) },
                cancel: { commands.cancelNewEntry() }
            )
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6)
        .frame(height: 22)
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
            .font(.system(size: 12))
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
