import SwiftUI

struct ProjectGroupSection: View {
    let group: ProjectGroup
    let projects: [Project]
    let isWide: Bool
    let draggedID: UUID?
    let globalIndexOffset: Int
    let onSelect: (Project) -> Void
    let onRemove: (Project) -> Void
    let onRename: (Project, String) -> Void
    let onSetLogo: (Project, String?) -> Void
    let onSetIconColor: (Project, String?) -> Void
    let onRenameGroup: (String) -> Void
    let onDeleteGroup: () -> Void
    let onAddGroup: () -> Void
    let onMoveProject: (Project, UUID) -> Void
    let allGroups: [ProjectGroup]
    let projectDragGesture: (Project) -> AnyGesture<DragGesture.Value>

    @Environment(ProjectGroupStore.self) private var groupStore

    @State private var isRenaming = false
    @State private var renameText = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            groupHeader
            if group.isExpanded {
                projectRows
            }
        }
        .padding(UIMetrics.spacing1)
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }

    private var isAnyDragging: Bool { draggedID != nil }

    private var groupHeader: some View {
        HStack(spacing: UIMetrics.spacing2) {
            if isWide {
                wideGroupHeader
            } else {
                collapsedGroupHeader
            }
        }
    }

    private var wideGroupHeader: some View {
        HStack(spacing: UIMetrics.spacing2) {
            if isRenaming {
                TextField("Group name", text: $renameText)
                    .textFieldStyle(.plain)
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .onSubmit { commitRename() }
                    .onExitCommand { cancelRename() }
            } else {
                Text(group.name.uppercased())
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .lineLimit(1)
            }
            Spacer()
            Image(systemName: group.isExpanded ? "chevron.down" : "chevron.right")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .padding(.horizontal, UIMetrics.spacing2)
        .padding(.vertical, UIMetrics.spacing1)
        .contentShape(Rectangle())
        .onTapGesture {
            withAnimation(.easeInOut(duration: 0.15)) {
                groupStore.toggleExpanded(groupID: group.id)
            }
        }
        .contextMenu { groupContextMenu }
    }

    private var collapsedGroupHeader: some View {
        VStack(spacing: UIMetrics.spacing1) {
            Text(group.name.prefix(1).uppercased())
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconXXL)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            Image(systemName: group.isExpanded ? "chevron.down" : "chevron.right")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconSM)
        }
        .contentShape(Rectangle())
        .onTapGesture {
            withAnimation(.easeInOut(duration: 0.15)) {
                groupStore.toggleExpanded(groupID: group.id)
            }
        }
        .contextMenu { groupContextMenu }
    }

    @ViewBuilder
    private var groupContextMenu: some View {
        Button("Add New Group") { onAddGroup() }
        Divider()
        Button("Rename Group") { startRename() }
        Button("Delete Group", role: .destructive) { onDeleteGroup() }
    }

    @ViewBuilder
    private var projectRows: some View {
        ForEach(Array(projects.enumerated()), id: \.element.id) { offset, project in
            let shortcutIndex = globalIndexOffset + offset
            Group {
                if isWide {
                    ExpandedProjectRow(
                        project: project,
                        shortcutIndex: shortcutIndex < 9 ? shortcutIndex + 1 : nil,
                        isAnyDragging: isAnyDragging,
                        onSelect: { onSelect(project) },
                        onRemove: { onRemove(project) },
                        onRename: { onRename(project, $0) },
                        onSetLogo: { onSetLogo(project, $0) },
                        onSetIconColor: { onSetIconColor(project, $0) }
                    )
                    .contextMenu { moveToGroupMenu(for: project) }
                } else {
                    ProjectRow(
                        project: project,
                        shortcutIndex: shortcutIndex < 9 ? shortcutIndex + 1 : nil,
                        isAnyDragging: isAnyDragging,
                        onSelect: { onSelect(project) },
                        onRemove: { onRemove(project) },
                        onRename: { onRename(project, $0) },
                        onSetLogo: { onSetLogo(project, $0) },
                        onSetIconColor: { onSetIconColor(project, $0) }
                    )
                    .contextMenu { moveToGroupMenu(for: project) }
                }
            }
            .background {
                if draggedID != nil {
                    GeometryReader { geo in
                        Color.clear.preference(
                            key: UUIDFramePreferenceKey<SidebarFrameTag>.self,
                            value: [project.id: geo.frame(in: .named("sidebar"))]
                        )
                    }
                }
            }
            .gesture(projectDragGesture(project))
        }
    }

    @ViewBuilder
    private func moveToGroupMenu(for project: Project) -> some View {
        let otherGroups = allGroups.filter { $0.id != group.id }
        if !otherGroups.isEmpty {
            Menu("Move to Group") {
                ForEach(otherGroups) { targetGroup in
                    Button(targetGroup.name) {
                        onMoveProject(project, targetGroup.id)
                    }
                }
            }
        }
    }

    private func startRename() {
        renameText = group.name
        isRenaming = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            cancelRename()
            return
        }
        onRenameGroup(trimmed)
        isRenaming = false
    }

    private func cancelRename() {
        renameText = ""
        isRenaming = false
    }
}
