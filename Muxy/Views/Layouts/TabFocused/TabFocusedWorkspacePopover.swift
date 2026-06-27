import SwiftUI

struct TabFocusedWorkspacePopover: View {
    let onDismiss: () -> Void

    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(RemoteDeviceStore.self) private var deviceStore
    @Environment(SSHConnectionService.self) private var sshConnections
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    private struct Item: Identifiable {
        let id: UUID?
        let name: String
        let group: ProjectGroup?
    }

    private var items: [Item] {
        [Item(id: nil, name: "All Projects", group: nil)]
            + projectGroupStore.groups.map { Item(id: $0.id, name: $0.name, group: $0) }
    }

    private var activeID: UUID? {
        projectGroupStore.activeGroupID
    }

    var body: some View {
        PopoverPicker(
            items: items,
            filterKey: { $0.name },
            searchPlaceholder: "Search workspaces…",
            emptyLabel: "No workspaces",
            onSelect: { select($0) },
            row: { item, isHighlighted in
                row(item, isHighlighted: isHighlighted)
            }
        )
    }

    private func row(_ item: Item, isHighlighted: Bool) -> some View {
        let selected = item.id == activeID
        return HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: item.group?.type == .ssh ? "network" : "square.stack.3d.up")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(selected ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(14))
            Text(item.name)
                .font(.system(size: UIMetrics.fontBody, weight: selected ? .semibold : .regular))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: UIMetrics.spacing1)
            if selected {
                Image(systemName: "checkmark")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
            }
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.scaled(7))
        .background(rowBackground(selected: selected, isHighlighted: isHighlighted), in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .padding(.horizontal, UIMetrics.spacing3)
        .padding(.vertical, UIMetrics.spacing1)
        .contentShape(Rectangle())
    }

    private func rowBackground(selected: Bool, isHighlighted: Bool) -> AnyShapeStyle {
        if selected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if isHighlighted { return AnyShapeStyle(MuxyTheme.surface) }
        return AnyShapeStyle(Color.clear)
    }

    private func select(_ item: Item) {
        guard let group = item.group else {
            projectGroupStore.clearGroupSelection()
            selectFirstProject()
            onDismiss()
            return
        }
        guard group.type == .ssh, let destination = deviceStore.device(id: group.remoteDeviceID)?.destination else {
            projectGroupStore.selectGroup(id: group.id)
            selectFirstProject()
            onDismiss()
            return
        }
        onDismiss()
        Task {
            guard await sshConnections.connect(destination: destination) else {
                ToastState.shared.show("Could not connect to \(group.name)")
                return
            }
            projectGroupStore.selectGroup(id: group.id)
            selectFirstProject()
        }
    }

    private func selectFirstProject() {
        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}
