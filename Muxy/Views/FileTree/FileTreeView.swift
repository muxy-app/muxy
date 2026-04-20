import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct FileTreeView: View {
    @Bindable var state: FileTreeState
    let onOpenFile: (String) -> Void
    let onOpenTerminal: (String) -> Void
    let onFileMoved: (String, String) -> Void

    @State private var commands: FileTreeCommands?
    @FocusState private var treeFocused: Bool

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
                                commands: commandsOrCreate(),
                                onOpenFile: onOpenFile,
                                requestFocus: { treeFocused = true }
                            )
                        }
                        if let pending = state.pendingNewEntry, pending.parentPath == normalizedRootPath {
                            FileTreeNewEntryRow(
                                kind: pending.kind,
                                depth: 0,
                                commands: commandsOrCreate()
                            )
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
            deleteAlertTitle,
            isPresented: Binding(
                get: { state.pendingDeletePath != nil },
                set: { newValue in
                    if !newValue { commandsOrCreate().cancelPendingDelete() }
                }
            ),
            presenting: state.pendingDeletePath
        ) { _ in
            Button("Move to Trash", role: .destructive) {
                commandsOrCreate().confirmPendingDelete()
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {
                commandsOrCreate().cancelPendingDelete()
            }
        } message: { path in
            Text("“\((path as NSString).lastPathComponent)” will be moved to the Trash.")
        }
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
                commands: commandsOrCreate()
            )
        }
    }

    private var emptySpaceTarget: some View {
        Color.clear
            .frame(maxWidth: .infinity)
            .containerRelativeFrame(.vertical)
            .contentShape(Rectangle())
            .onTapGesture {
                state.selectedFilePath = nil
                treeFocused = true
            }
            .contextMenu {
                FileTreeContextMenuContents(
                    path: state.rootPath,
                    isDirectory: true,
                    includesTargetActions: false,
                    commands: commandsOrCreate()
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
                    commands: commandsOrCreate()
                )
            )
    }

    private var keyboardShortcuts: some View {
        Group {
            Button("") {
                guard let path = state.selectedFilePath else { return }
                commandsOrCreate().beginRename(path: path)
            }
            .keyboardShortcut(.return, modifiers: [])

            Button("") {
                guard let path = state.selectedFilePath else { return }
                commandsOrCreate().trash(path: path)
            }
            .keyboardShortcut(.delete, modifiers: [])

            Button("") {
                guard let path = state.selectedFilePath else { return }
                commandsOrCreate().trash(path: path)
            }
            .keyboardShortcut(.delete, modifiers: [.command])

            Button("") {
                guard let path = state.selectedFilePath else { return }
                commandsOrCreate().copyToClipboard(paths: [path])
            }
            .keyboardShortcut("c", modifiers: [.command])

            Button("") {
                guard let path = state.selectedFilePath else { return }
                commandsOrCreate().cutToClipboard(paths: [path])
            }
            .keyboardShortcut("x", modifiers: [.command])

            Button("") {
                let target = state.selectedFilePath ?? state.rootPath
                commandsOrCreate().paste(into: target)
            }
            .keyboardShortcut("v", modifiers: [.command])
        }
        .buttonStyle(.plain)
        .opacity(0)
        .frame(width: 0, height: 0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private var deleteAlertTitle: String {
        guard let path = state.pendingDeletePath else { return "Move to Trash?" }
        var isDir: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: path, isDirectory: &isDir)
        let kind = exists && isDir.boolValue ? "folder" : "file"
        return "Move \(kind) to Trash?"
    }

    private var normalizedRootPath: String {
        state.rootPath.hasSuffix("/") ? String(state.rootPath.dropLast()) : state.rootPath
    }

    private func commandsOrCreate() -> FileTreeCommands {
        if let commands { return commands }
        let created = FileTreeCommands(
            state: state,
            openTerminal: onOpenTerminal,
            onFileMoved: onFileMoved
        )
        commands = created
        return created
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
        !entry.isDirectory && state.selectedFilePath == entry.absolutePath
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
            chevron
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
        .padding(.horizontal, 8)
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

    @ViewBuilder
    private var chevron: some View {
        if entry.isDirectory {
            Image(systemName: state.isExpanded(entry) ? "chevron.down" : "chevron.right")
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(width: 10)
        } else {
            Color.clear.frame(width: 10)
        }
    }

    private var icon: some View {
        Image(systemName: entry.isDirectory ? "folder" : "doc")
            .font(.system(size: 11))
            .foregroundStyle(iconColor)
            .frame(width: 14)
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
        state.selectedFilePath = entry.absolutePath
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
            Color.clear.frame(width: 10)
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
        .padding(.horizontal, 8)
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
                DispatchQueue.main.async { focused = true }
            }
            .onSubmit {
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !trimmed.isEmpty else { cancel()
                    return
                }
                commit(trimmed)
            }
            .onExitCommand { cancel() }
            .onChange(of: focused) { _, isFocused in
                guard didAppear, !isFocused else { return }
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if trimmed.isEmpty || trimmed == initialName {
                    cancel()
                } else {
                    commit(trimmed)
                }
            }
    }
}

private struct FileTreeContextMenuContents: View {
    let path: String
    let isDirectory: Bool
    let includesTargetActions: Bool
    let commands: FileTreeCommands

    var body: some View {
        Button("New File") { commands.beginNewFile(in: path) }
        Button("New Folder") { commands.beginNewFolder(in: path) }
        if includesTargetActions {
            Divider()
            Button("Rename") { commands.beginRename(path: path) }
            Button("Delete") { commands.trash(path: path) }
            Divider()
            Button("Cut") { commands.cutToClipboard(paths: [path]) }
            Button("Copy") { commands.copyToClipboard(paths: [path]) }
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
