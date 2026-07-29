import SwiftUI

enum TabFocusedSidebarContent: Equatable {
    case tabs
    case agents
}

struct TabFocusedSidebar: View {
    var content: TabFocusedSidebarContent = .tabs

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var expansionStore = TabFocusedSidebarState.shared
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage(ProjectSortMode.storageKey) private var sortModeRaw = ProjectSortMode.defaultValue.rawValue
    @AppStorage(WorktreeListPreferences.groupWorktreesKey)
    private var groupWorktrees = WorktreeListPreferences.defaultGroupWorktrees
    @AppStorage(SidebarExpandedStyle.storageKey) private var expandedStyleRaw = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage("muxy.sidebarExpanded") private var sidebarExpanded = true

    private var expandedStyle: SidebarExpandedStyle {
        SidebarExpandedStyle(rawValue: expandedStyleRaw) ?? .defaultValue
    }

    private var isWide: Bool {
        sidebarExpanded && expandedStyle == .wide
    }

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
            activeProjectID: appState.activeProjectID,
            content: content
        )
    }

    private var rows: [TabFocusedSidebarRowItem] {
        TabFocusedSidebarRows.resolve(
            projects: projects,
            content: content,
            groupWorktrees: groupWorktrees,
            worktreesForProject: { worktreeStore.list(for: $0) },
            hasTabs: { appState.hasTabs(for: $0) }
        )
    }

    private var shortcutNumbers: [UUID: Int] {
        guard content == .tabs else { return [:] }
        let entries = TabFocusedTabOrder.entries(
            appState: appState,
            projectStore: projectStore,
            projectGroupStore: projectGroupStore,
            worktreeStore: worktreeStore,
            groupWorktrees: groupWorktrees
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
            HStack(spacing: UIMetrics.spacing2) {
                WorkspaceSwitcher(isWide: isWide)
                if isWide {
                    sortMenu
                }
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.top, UIMetrics.spacing2)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(rows) { row in
                        TabFocusedProjectRow(
                            project: row.project,
                            worktree: row.worktree,
                            shortcutNumbers: numbers,
                            content: content,
                            groupWorktrees: groupWorktrees
                        )
                    }
                    if content == .agents || !expansionStore.focusMode {
                        TabFocusedAddProjectRow(
                            recentlyRemovedProjects: displayedRecentlyRemovedProjects,
                            openProject: openProjectPicker,
                            restoreProject: restoreRecentlyRemovedProject
                        )
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

    private var sortMenu: some View {
        Menu {
            Picker(L10n.string("Sort Projects By"), selection: $sortModeRaw) {
                ForEach(ProjectSortMode.allCases) { mode in
                    Label(L10n.string(key: mode.title), systemImage: mode.systemImage).tag(mode.rawValue)
                }
            }
            .pickerStyle(.inline)
        } label: {
            SidebarHeaderIconButtonLabel(
                systemName: "arrow.up.arrow.down",
                accessibilityLabel: L10n.string("Sort Projects")
            )
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
        .help(L10n.string("Sort Projects: \(L10n.string(key: sortMode.title))"))
    }

    private func openProjectPicker() {
        ProjectOpenService.openProjectViaPicker(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }

    private var displayedRecentlyRemovedProjects: [RecentlyRemovedProject] {
        guard !projectGroupStore.isRemoteWorkspaceActive else { return [] }
        return projectStore.recentlyRemovedProjects
    }

    private func restoreRecentlyRemovedProject(id: UUID) {
        do {
            try ProjectOpenService.restoreRecentlyRemovedProject(
                id: id,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        } catch {
            ToastState.shared.show(title: L10n.string("Could not restore project"), body: error.localizedDescription)
        }
    }
}

struct AgentsFocusedSidebar: View {
    var body: some View {
        TabFocusedSidebar(content: .agents)
    }
}

enum TabFocusedSidebarProjectSelection {
    static func resolve(
        projects: [Project],
        focusMode: Bool,
        activeProjectID: UUID?,
        content: TabFocusedSidebarContent = .tabs
    ) -> [Project] {
        guard content == .tabs,
              focusMode,
              let activeProjectID,
              let activeProject = projects.first(where: { $0.id == activeProjectID })
        else { return projects }
        return [activeProject]
    }
}

enum TabFocusedSidebarRows {
    static func resolve(
        projects: [Project],
        content: TabFocusedSidebarContent,
        groupWorktrees: Bool,
        worktreesForProject: (UUID) -> [Worktree],
        hasTabs: (WorktreeKey) -> Bool
    ) -> [TabFocusedSidebarRowItem] {
        projects.flatMap { project in
            var rows: [TabFocusedSidebarRowItem] = [.project(project)]
            guard !groupWorktrees, project.worktreesEnabled, !project.isHome else { return rows }
            for worktree in worktreesForProject(project.id) where !worktree.isPrimary {
                let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
                guard content == .agents || hasTabs(key) else { continue }
                rows.append(.worktree(project, worktree))
            }
            return rows
        }
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
    let recentlyRemovedProjects: [RecentlyRemovedProject]
    let openProject: () -> Void
    let restoreProject: (UUID) -> Void
    @State private var hovered = false

    var body: some View {
        if recentlyRemovedProjects.isEmpty {
            Button(action: openProject) {
                label
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.string("Add Project"))
        } else {
            Menu {
                Button(action: openProject) {
                    Label(L10n.string("Choose Folder…"), systemImage: "folder")
                }
                Divider()
                Section(L10n.string("Recently Removed")) {
                    ForEach(recentlyRemovedProjects) { entry in
                        Button {
                            restoreProject(entry.id)
                        } label: {
                            Label(entry.project.name, systemImage: entry.project.icon ?? "folder")
                        }
                    }
                }
            } label: {
                label
            }
            .menuStyle(.button)
            .menuIndicator(.hidden)
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.string("Add Project"))
        }
    }

    private var label: some View {
        HStack(spacing: TabFocusedSidebarMetrics.iconTitleGap) {
            Image(systemName: "plus")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(
                    width: TabFocusedSidebarMetrics.folderIconSize,
                    height: TabFocusedSidebarMetrics.folderIconSize
                )
            Text(L10n.resource("Add Project"))
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
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .padding(.vertical, TabFocusedSidebarMetrics.rowVerticalPadding)
        .onHover { hovered = $0 }
        .help(shortcutTooltip)
    }

    private var shortcutTooltip: String {
        "Add Project (\(KeyBindingStore.shared.combo(for: .openProject).displayString))"
    }
}
