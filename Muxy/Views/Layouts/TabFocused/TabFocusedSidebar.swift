import SwiftUI

struct TabFocusedSidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var expansionStore = TabFocusedSidebarState.shared
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage(ProjectSortMode.storageKey) private var sortModeRaw = ProjectSortMode.defaultValue.rawValue

    private var sortMode: ProjectSortMode {
        ProjectSortMode(rawValue: sortModeRaw) ?? .defaultValue
    }

    private var homeProject: Project? {
        guard showHomeProject else { return nil }
        guard !projectGroupStore.isRemoteWorkspaceActive else {
            return projectGroupStore.activeRemoteHomeProject
        }
        return Project.home
    }

    private var projects: [Project] {
        let stored = projectGroupStore.displayProjects(localProjects: projectStore.storedProjects, sortMode: sortMode)
        let all = homeProject.map { [$0] + stored } ?? stored
        return TabFocusedSidebarProjectSelection.resolve(
            projects: all,
            focusMode: expansionStore.focusMode,
            activeProjectID: appState.activeProjectID
        )
    }

    private var rows: [TabFocusedSidebarRowItem] {
        projects.flatMap { project -> [TabFocusedSidebarRowItem] in
            var items: [TabFocusedSidebarRowItem] = [.project(project)]
            guard project.worktreesEnabled, !project.isHome else { return items }
            for worktree in worktreeStore.list(for: project.id) where !worktree.isPrimary {
                let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
                guard appState.hasTabs(for: key) else { continue }
                items.append(.worktree(project, worktree))
            }
            return items
        }
    }

    private var shortcutNumbers: [UUID: Int] {
        let entries = TabFocusedTabOrder.entries(
            appState: appState,
            projectStore: projectStore,
            projectGroupStore: projectGroupStore,
            worktreeStore: worktreeStore
        )
        var map: [UUID: Int] = [:]
        for (index, entry) in entries.prefix(9).enumerated() {
            map[entry.tabID] = index + 1
        }
        return map
    }

    var body: some View {
        let numbers = shortcutNumbers
        return VStack(spacing: 0) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(rows) { row in
                        TabFocusedProjectRow(
                            project: row.project,
                            worktree: row.worktree,
                            shortcutNumbers: numbers
                        )
                    }
                    if !expansionStore.focusMode {
                        TabFocusedAddProjectRow(action: openProjectPicker)
                    }
                }
                .padding(.top, UIMetrics.spacing5)
                .padding(.bottom, UIMetrics.spacing3)
            }
            .scrollIndicators(.never)

            SidebarFooter()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }

    private func openProjectPicker() {
        ProjectOpenService.openProjectViaPicker(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}

enum TabFocusedSidebarProjectSelection {
    static func resolve(projects: [Project], focusMode: Bool, activeProjectID: UUID?) -> [Project] {
        guard focusMode,
              let activeProjectID,
              let activeProject = projects.first(where: { $0.id == activeProjectID })
        else { return projects }
        return [activeProject]
    }
}

enum TabFocusedSidebarRowItem: Identifiable {
    case project(Project)
    case worktree(Project, Worktree)

    var id: UUID {
        switch self {
        case let .project(project): project.id
        case let .worktree(_, worktree): worktree.id
        }
    }

    var project: Project {
        switch self {
        case let .project(project): project
        case let .worktree(project, _): project
        }
    }

    var worktree: Worktree? {
        switch self {
        case .project: nil
        case let .worktree(_, worktree): worktree
        }
    }
}

private struct TabFocusedAddProjectRow: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: TabFocusedSidebarMetrics.iconTitleGap) {
                Image(systemName: "plus")
                    .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                    .frame(
                        width: TabFocusedSidebarMetrics.folderIconSize,
                        height: TabFocusedSidebarMetrics.folderIconSize
                    )
                Text("Add Project")
                    .font(.system(size: UIMetrics.fontHeadline, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, TabFocusedSidebarMetrics.rowHorizontalInset)
            .frame(minHeight: TabFocusedSidebarMetrics.rowHeight)
            .background {
                RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                    .fill(hovered ? MuxyTheme.hover : Color.clear)
            }
            .contentShape(RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous))
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .padding(.vertical, TabFocusedSidebarMetrics.rowVerticalPadding)
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(shortcutTooltip)
        .accessibilityLabel("Add Project")
    }

    private var shortcutTooltip: String {
        "Add Project (\(KeyBindingStore.shared.combo(for: .openProject).displayString))"
    }
}
