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
    @State private var projectDragState = AgentsFocusedProjectDragState()

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
                VStack(alignment: .leading, spacing: 0) {
                    if content == .agents {
                        agentsFocusedProjectBlocks(numbers: numbers)
                    } else {
                        ForEach(rows) { row in
                            TabFocusedProjectRow(
                                project: row.project,
                                worktree: row.worktree,
                                shortcutNumbers: numbers,
                                content: content,
                                groupWorktrees: groupWorktrees
                            )
                        }
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
                .onPreferenceChange(UUIDFramePreferenceKey<AgentsFocusedProjectFrameTag>.self) { frames in
                    guard projectDragState.draggedID != nil else { return }
                    projectDragState.frames = frames
                }
                .onChange(of: reorderableProjects.map(\.id)) { _, projectIDs in
                    guard let draggedID = projectDragState.draggedID,
                          !projectIDs.contains(draggedID)
                    else { return }
                    handleProjectDragEnded()
                }
            }
            .scrollIndicators(.never)
            .coordinateSpace(name: AgentsFocusedProjectDragCoordinateSpace.sidebar)

            SidebarFooter(isWide: isWide, sidebarExpanded: sidebarExpanded)
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

    private func agentsFocusedProjectBlocks(numbers: [UUID: Int]) -> some View {
        ForEach(projects) { project in
            agentsFocusedProjectBlock(project, numbers: numbers)
        }
    }

    private func agentsFocusedProjectBlock(_ project: Project, numbers: [UUID: Int]) -> some View {
        let reorderable = isProjectReorderable(project)
        let onDragChanged: ((Project, CGPoint) -> Void)? = reorderable
            ? { _, location in handleProjectDragChanged(project: project, location: location) }
            : nil
        let onDragEnded: (() -> Void)? = reorderable ? { handleProjectDragEnded() } : nil

        return AgentsFocusedProjectBlock(
            project: project,
            standaloneWorktrees: standaloneWorktrees(for: project),
            shortcutNumbers: numbers,
            groupWorktrees: groupWorktrees,
            onProjectHeaderDragChanged: onDragChanged,
            onProjectHeaderDragEnded: onDragEnded
        )
        .opacity(projectDragState.draggedID == project.id ? 0.5 : 1)
        .background {
            if projectDragState.draggedID != nil, reorderable {
                GeometryReader { geo in
                    Color.clear.preference(
                        key: UUIDFramePreferenceKey<AgentsFocusedProjectFrameTag>.self,
                        value: [
                            project.id: geo.frame(
                                in: .named(AgentsFocusedProjectDragCoordinateSpace.sidebar)
                            ),
                        ]
                    )
                }
            }
        }
    }

    private func standaloneWorktrees(for project: Project) -> [Worktree] {
        TabFocusedSidebarRows.standaloneWorktrees(
            project: project,
            content: content,
            groupWorktrees: groupWorktrees,
            worktrees: worktreeStore.list(for: project.id),
            hasTabs: { appState.hasTabs(for: $0) }
        )
    }

    private func isProjectReorderable(_ project: Project) -> Bool {
        sortMode == .manual
            && !project.isHome
            && !project.isRemote
            && !projectGroupStore.isRemoteWorkspaceActive
            && projectStore.storedProjects.contains(where: { $0.id == project.id })
    }

    private var reorderableProjects: [Project] {
        projects.filter(isProjectReorderable)
    }

    private func handleProjectDragChanged(project: Project, location: CGPoint) {
        guard isProjectReorderable(project) else { return }
        if projectDragState.draggedID == nil {
            projectDragState.draggedID = project.id
            projectDragState.lastReorderTargetID = nil
        }
        reorderProjectsIfNeeded(at: location)
    }

    private func handleProjectDragEnded() {
        withAnimation(.easeInOut(duration: 0.15)) {
            projectDragState.draggedID = nil
            projectDragState.frames = [:]
            projectDragState.lastReorderTargetID = nil
        }
    }

    private func reorderProjectsIfNeeded(at location: CGPoint) {
        let candidates = reorderableProjects
        guard let draggedID = projectDragState.draggedID else { return }
        var hoveredTargetID: UUID?

        for (id, frame) in projectDragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            hoveredTargetID = id
            guard projectDragState.lastReorderTargetID != id,
                  let sourceIndex = candidates.firstIndex(where: { $0.id == draggedID }),
                  let destinationIndex = candidates.firstIndex(where: { $0.id == id }),
                  candidates[sourceIndex].isPinned == candidates[destinationIndex].isPinned
            else { return }

            projectDragState.lastReorderTargetID = id
            var reorderedProjects = candidates
            reorderedProjects.move(
                fromOffsets: IndexSet(integer: sourceIndex),
                toOffset: destinationIndex > sourceIndex ? destinationIndex + 1 : destinationIndex
            )
            withAnimation(.easeInOut(duration: 0.15)) {
                projectStore.persistOrder(
                    reorderedProjects.map(\.id),
                    scopedTo: Set(reorderedProjects.map(\.id))
                )
            }
            return
        }

        if hoveredTargetID == nil {
            projectDragState.lastReorderTargetID = nil
        }
    }
}

private struct AgentsFocusedProjectBlock: View {
    let project: Project
    let standaloneWorktrees: [Worktree]
    let shortcutNumbers: [UUID: Int]
    let groupWorktrees: Bool
    let onProjectHeaderDragChanged: ((Project, CGPoint) -> Void)?
    let onProjectHeaderDragEnded: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            TabFocusedProjectRow(
                project: project,
                shortcutNumbers: shortcutNumbers,
                content: .agents,
                groupWorktrees: groupWorktrees,
                onProjectHeaderDragChanged: onProjectHeaderDragChanged,
                onProjectHeaderDragEnded: onProjectHeaderDragEnded
            )
            ForEach(standaloneWorktrees) { worktree in
                TabFocusedProjectRow(
                    project: project,
                    worktree: worktree,
                    shortcutNumbers: shortcutNumbers,
                    content: .agents,
                    groupWorktrees: groupWorktrees
                )
            }
        }
    }
}

private struct AgentsFocusedProjectDragState {
    var draggedID: UUID?
    var frames: [UUID: CGRect] = [:]
    var lastReorderTargetID: UUID?
}

enum AgentsFocusedProjectDragCoordinateSpace {
    static let sidebar = "AgentsFocusedProjectSidebar"
}

private enum AgentsFocusedProjectFrameTag {}

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
    static func standaloneWorktrees(
        project: Project,
        content: TabFocusedSidebarContent,
        groupWorktrees: Bool,
        worktrees: [Worktree],
        hasTabs: (WorktreeKey) -> Bool
    ) -> [Worktree] {
        guard !groupWorktrees, project.worktreesEnabled, !project.isHome else { return [] }
        return worktrees.filter { worktree in
            guard !worktree.isPrimary else { return false }
            guard content == .tabs else { return true }
            return hasTabs(WorktreeKey(projectID: project.id, worktreeID: worktree.id))
        }
    }

    static func resolve(
        projects: [Project],
        content: TabFocusedSidebarContent,
        groupWorktrees: Bool,
        worktreesForProject: (UUID) -> [Worktree],
        hasTabs: (WorktreeKey) -> Bool
    ) -> [TabFocusedSidebarRowItem] {
        projects.flatMap { project in
            let standalone = standaloneWorktrees(
                project: project,
                content: content,
                groupWorktrees: groupWorktrees,
                worktrees: worktreesForProject(project.id),
                hasTabs: hasTabs
            )
            let worktreeRows = standalone.map { TabFocusedSidebarRowItem.worktree(project, $0) }
            return [TabFocusedSidebarRowItem.project(project)] + worktreeRows
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
