import SwiftUI

struct FileTreeView: View {
    @Bindable var state: FileTreeState
    let onOpenFile: (String) -> Void

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(state.visibleRootEntries(), id: \.absolutePath) { entry in
                        FileTreeRowGroup(entry: entry, depth: 0, state: state, onOpenFile: onOpenFile)
                    }
                }
                .padding(.vertical, 4)
            }
        }
        .background(MuxyTheme.bg)
        .task(id: state.rootPath) {
            state.loadRootIfNeeded()
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
    }
}

private struct FileTreeRowGroup: View {
    let entry: FileTreeEntry
    let depth: Int
    @Bindable var state: FileTreeState
    let onOpenFile: (String) -> Void

    var body: some View {
        FileTreeRow(entry: entry, depth: depth, state: state, onOpenFile: onOpenFile)
        if entry.isDirectory, state.isExpanded(entry), let children = state.visibleChildren(of: entry) {
            ForEach(children, id: \.absolutePath) { child in
                FileTreeRowGroup(entry: child, depth: depth + 1, state: state, onOpenFile: onOpenFile)
            }
        }
    }
}

private struct FileTreeRow: View {
    let entry: FileTreeEntry
    let depth: Int
    @Bindable var state: FileTreeState
    let onOpenFile: (String) -> Void
    @State private var hovered = false

    private var isSelected: Bool {
        !entry.isDirectory && state.selectedFilePath == entry.absolutePath
    }

    var body: some View {
        HStack(spacing: 4) {
            Color.clear.frame(width: CGFloat(depth) * 12)
            chevron
            icon
            Text(entry.name)
                .font(.system(size: 12))
                .foregroundStyle(textColor)
                .lineLimit(1)
                .truncationMode(.tail)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 8)
        .frame(height: 22)
        .opacity(entry.isIgnored ? 0.45 : 1)
        .background(rowBackground)
        .contentShape(Rectangle())
        .onTapGesture { handleTap() }
        .onHover { hovered = $0 }
    }

    private var rowBackground: Color {
        if isSelected { return MuxyTheme.accentSoft }
        if hovered { return MuxyTheme.hover }
        return .clear
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
        if entry.isDirectory {
            state.toggle(entry)
        } else if state.status(for: entry.absolutePath) != .deleted {
            onOpenFile(entry.absolutePath)
        } else {
            return
        }
    }
}
