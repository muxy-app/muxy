import SwiftUI

struct WorkspaceSwitcher: View {
    let isWide: Bool

    @Environment(ProjectGroupStore.self) private var projectGroupStore

    @State private var isShowingPopover = false
    @State private var isTriggerHovered = false
    @State private var editorMode: WorkspaceEditorMode?
    @State private var groupPendingDelete: ProjectGroup?

    private var activeGroup: ProjectGroup? {
        guard let id = projectGroupStore.activeGroupID else { return nil }
        return projectGroupStore.groups.first(where: { $0.id == id })
    }

    private var activeLabel: String {
        activeGroup?.name ?? "All Projects"
    }

    var body: some View {
        Group {
            if isWide {
                wideLayout
            } else {
                collapsedLayout
            }
        }
        .sheet(item: $editorMode) { mode in
            WorkspaceEditorSheet(
                mode: mode,
                onSubmit: { name in
                    apply(mode: mode, name: name)
                    editorMode = nil
                },
                onCancel: { editorMode = nil }
            )
        }
        .alert(
            "Delete “\(groupPendingDelete?.name ?? "")”?",
            isPresented: deleteAlertBinding,
            presenting: groupPendingDelete
        ) { group in
            Button("Delete", role: .destructive) {
                projectGroupStore.removeGroup(id: group.id)
                groupPendingDelete = nil
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {
                groupPendingDelete = nil
            }
        } message: { _ in
            Text("Projects in this workspace will not be deleted.")
        }
    }

    private var wideLayout: some View {
        Button {
            isShowingPopover.toggle()
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Text(activeLabel)
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .lineLimit(1)
                Spacer()
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.vertical, UIMetrics.spacing3)
            .background(
                isTriggerHovered ? MuxyTheme.hover : MuxyTheme.surface,
                in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isTriggerHovered = $0 }
        .popover(isPresented: $isShowingPopover, arrowEdge: .bottom) {
            workspacePopover
        }
    }

    private var collapsedLayout: some View {
        Button {
            isShowingPopover.toggle()
        } label: {
            Image(systemName: "chevron.down")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
                .background(
                    isTriggerHovered ? MuxyTheme.hover : MuxyTheme.surface,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                )
        }
        .buttonStyle(.plain)
        .onHover { isTriggerHovered = $0 }
        .popover(isPresented: $isShowingPopover, arrowEdge: .trailing) {
            workspacePopover
        }
    }

    private var workspacePopover: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
            allProjectsRow
            Divider()
                .padding(.vertical, UIMetrics.spacing1)
            ForEach(projectGroupStore.groups) { group in
                WorkspaceRow(
                    group: group,
                    isActive: projectGroupStore.activeGroupID == group.id,
                    onSelect: {
                        projectGroupStore.selectGroup(id: group.id)
                        isShowingPopover = false
                    },
                    onRename: {
                        isShowingPopover = false
                        editorMode = .rename(group)
                    },
                    onDelete: {
                        isShowingPopover = false
                        groupPendingDelete = group
                    }
                )
            }
            if !projectGroupStore.groups.isEmpty {
                Divider()
                    .padding(.vertical, UIMetrics.spacing1)
            }
            newWorkspaceButton
        }
        .padding(UIMetrics.spacing3)
        .frame(minWidth: 180)
    }

    private var allProjectsRow: some View {
        Button {
            projectGroupStore.clearGroupSelection()
            isShowingPopover = false
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: projectGroupStore.activeGroupID == nil ? "checkmark" : "")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
                    .frame(width: UIMetrics.fontCaption)
                Text("All Projects")
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer()
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var newWorkspaceButton: some View {
        Button {
            isShowingPopover = false
            editorMode = .create
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "plus")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
                    .frame(width: UIMetrics.fontCaption)
                Text("New Workspace")
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer()
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var deleteAlertBinding: Binding<Bool> {
        Binding(
            get: { groupPendingDelete != nil },
            set: { newValue in
                if !newValue {
                    groupPendingDelete = nil
                }
            }
        )
    }

    private func apply(mode: WorkspaceEditorMode, name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        switch mode {
        case .create:
            projectGroupStore.addGroup(name: trimmed)
        case let .rename(group):
            projectGroupStore.renameGroup(id: group.id, to: trimmed)
        }
    }
}

enum WorkspaceEditorMode: Identifiable {
    case create
    case rename(ProjectGroup)

    var id: String {
        switch self {
        case .create: "create"
        case let .rename(group): "rename-\(group.id.uuidString)"
        }
    }

    var title: String {
        switch self {
        case .create: "New Workspace"
        case .rename: "Rename Workspace"
        }
    }

    var actionLabel: String {
        switch self {
        case .create: "Create"
        case .rename: "Rename"
        }
    }

    var initialName: String {
        switch self {
        case .create: ""
        case let .rename(group): group.name
        }
    }
}

struct ProjectGroupMembershipMenu: View {
    let project: Project

    @Environment(ProjectGroupStore.self) private var projectGroupStore

    var body: some View {
        Menu("Move to Workspace") {
            ForEach(projectGroupStore.groups) { group in
                let isInGroup = group.projectIDs.contains(project.id)
                Button {
                    if isInGroup {
                        projectGroupStore.removeProject(projectID: project.id, fromGroup: group.id)
                    } else {
                        projectGroupStore.addProject(projectID: project.id, toGroup: group.id)
                    }
                } label: {
                    Label(group.name, systemImage: isInGroup ? "checkmark" : "")
                }
            }
        }
    }
}

private struct WorkspaceRow: View {
    let group: ProjectGroup
    let isActive: Bool
    let onSelect: () -> Void
    let onRename: () -> Void
    let onDelete: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: isActive ? "checkmark" : "")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
                    .frame(width: UIMetrics.fontCaption)
                Text(group.name)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                Spacer()
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .background(isHovered ? MuxyTheme.hover : Color.clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
        .contextMenu {
            Button("Rename", action: onRename)
            Button("Delete", role: .destructive, action: onDelete)
        }
    }
}

private struct WorkspaceEditorSheet: View {
    let mode: WorkspaceEditorMode
    let onSubmit: (String) -> Void
    let onCancel: () -> Void

    @State private var name: String = ""
    @FocusState private var nameFocused: Bool

    private var trimmed: String {
        name.trimmingCharacters(in: .whitespaces)
    }

    private var canSubmit: Bool {
        !trimmed.isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(mode.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Workspace Name")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                TextField("Personal", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .focused($nameFocused)
                    .onSubmit { if canSubmit { onSubmit(trimmed) } }
            }

            HStack {
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(mode.actionLabel) { onSubmit(trimmed) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canSubmit)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(360))
        .onAppear {
            name = mode.initialName
            nameFocused = true
        }
    }
}
