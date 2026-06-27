import SwiftUI

struct TabFocusedBreadcrumb: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    private static let originKey = "muxy.breadcrumb.origin"
    private static let originID = "tabFocusedBreadcrumb"

    @State private var currentBranch: String?
    @State private var branches: [String] = []
    @State private var showWorkspacePopover = false
    @State private var showProjectPopover = false
    @State private var showBranchPopover = false
    @State private var showWorktreePopover = false
    @State private var showCreateSheet = false
    @State private var isSwitching = false

    private var activeProject: Project? {
        guard let id = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == id }
    }

    private var workspaceName: String {
        projectGroupStore.activeGroup?.name ?? "All Projects"
    }

    private func activeWorktree(for project: Project) -> Worktree? {
        worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id])
    }

    private var resolvedProject: Project? {
        guard let project = activeProject, !project.isHome else { return nil }
        return project
    }

    var body: some View {
        let project = resolvedProject
        let worktree = project.flatMap(activeWorktree)
        return HStack(spacing: UIMetrics.spacing2) {
            workspaceSegment

            separator
            projectSegment(project: project)

            if let project {
                separator
                worktreeSegment(project: project, worktree: worktree)

                if currentBranch != nil {
                    separator
                    branchSegment(project: project)
                }
            }
        }
        .fixedSize(horizontal: true, vertical: false)
        .padding(.leading, UIMetrics.spacing6)
        .task(id: taskID(project: project, worktree: worktree)) {
            await loadBranches(project: project, worktree: worktree)
        }
        .onReceive(NotificationCenter.default.publisher(for: .vcsDidRefresh)) { notification in
            guard !isSelfOriginated(notification, project: project, worktree: worktree) else { return }
            Task { await loadBranches(project: project, worktree: worktree) }
        }
        .onReceive(NotificationCenter.default.publisher(for: .vcsRepoDidChange)) { _ in
            Task { await loadBranches(project: project, worktree: worktree) }
        }
    }

    private var separator: some View {
        Text("/")
            .font(.system(size: UIMetrics.fontBody, weight: .regular))
            .foregroundStyle(MuxyTheme.fgDim)
    }

    private var workspaceSegment: some View {
        BreadcrumbSegment(
            symbol: "square.stack.3d.up",
            text: workspaceName,
            busy: false,
            isOpen: showWorkspacePopover,
            action: { showWorkspacePopover = true }
        )
        .popover(isPresented: $showWorkspacePopover, arrowEdge: .bottom) {
            TabFocusedWorkspacePopover(onDismiss: { showWorkspacePopover = false })
        }
    }

    private func projectSegment(project: Project?) -> some View {
        BreadcrumbSegment(
            symbol: "folder",
            text: project?.name ?? "Select Project",
            busy: false,
            isOpen: showProjectPopover,
            action: { showProjectPopover = true }
        )
        .popover(isPresented: $showProjectPopover, arrowEdge: .bottom) {
            TabFocusedProjectPopover(onDismiss: { showProjectPopover = false })
        }
    }

    private func branchSegment(project: Project) -> some View {
        BreadcrumbSegment(
            symbol: "arrow.triangle.branch",
            text: currentBranch ?? "",
            busy: isSwitching,
            isOpen: showBranchPopover,
            action: { showBranchPopover = true }
        )
        .popover(isPresented: $showBranchPopover, arrowEdge: .bottom) {
            TabFocusedBranchPopover(
                project: project,
                currentBranch: currentBranch,
                branches: branches,
                onSwitch: { branch in switchBranch(project: project, branch: branch) },
                onDismiss: { showBranchPopover = false }
            )
        }
    }

    private func worktreeSegment(project: Project, worktree: Worktree?) -> some View {
        BreadcrumbSegment(
            symbol: "point.3.connected.trianglepath.dotted",
            text: worktreeName(worktree),
            busy: false,
            isOpen: showWorktreePopover,
            action: { showWorktreePopover = true }
        )
        .popover(isPresented: $showWorktreePopover, arrowEdge: .bottom) {
            WorktreePopover(
                project: project,
                isGitRepo: currentBranch != nil,
                onDismiss: { showWorktreePopover = false },
                onRequestCreate: {
                    showWorktreePopover = false
                    showCreateSheet = true
                },
                onRequestRemove: { worktree in removeWorktree(project: project, worktree: worktree) }
            )
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateWorktreeSheet(project: project) { result in
                showCreateSheet = false
                handleCreateResult(project: project, result: result)
            }
        }
    }

    private func worktreeName(_ worktree: Worktree?) -> String {
        guard let worktree else { return "worktree" }
        if worktree.isPrimary, worktree.name.isEmpty { return "main" }
        return worktree.name
    }

    private func taskID(project: Project?, worktree: Worktree?) -> String {
        guard let project else { return "none" }
        return "\(worktree?.path ?? project.path)|\(project.isRemote)"
    }

    private func repoPath(project: Project, worktree: Worktree?) -> String {
        worktree?.path ?? project.path
    }

    private func loadBranches(project: Project?, worktree: Worktree?) async {
        guard let project else {
            currentBranch = nil
            branches = []
            return
        }
        let context = projectGroupStore.workspaceContext(for: project)
        let service = GitRepositoryService(context: context)
        let path = repoPath(project: project, worktree: worktree)
        guard await GitWorktreeService.shared.isGitRepository(path, context: context) else {
            currentBranch = nil
            branches = []
            return
        }
        currentBranch = try? await service.currentBranch(repoPath: path)
        branches = await (try? service.listBranches(repoPath: path)) ?? []
    }

    private func switchBranch(project: Project, branch: String) {
        let context = projectGroupStore.workspaceContext(for: project)
        let service = GitRepositoryService(context: context)
        let path = repoPath(project: project, worktree: activeWorktree(for: project))
        isSwitching = true
        Task {
            defer { isSwitching = false }
            do {
                try await service.switchBranch(repoPath: path, branch: branch)
                currentBranch = branch
                NotificationCenter.default.post(
                    name: .vcsDidRefresh,
                    object: nil,
                    userInfo: ["repoPath": path, Self.originKey: Self.originID]
                )
            } catch {
                ToastState.shared.show(title: "Failed to switch branch", body: error.localizedDescription)
            }
        }
    }

    private func isSelfOriginated(_ notification: Notification, project: Project?, worktree: Worktree?) -> Bool {
        guard let project else { return false }
        guard notification.userInfo?[Self.originKey] as? String == Self.originID else { return false }
        return notification.userInfo?["repoPath"] as? String == repoPath(project: project, worktree: worktree)
    }

    private func handleCreateResult(project: Project, result: CreateWorktreeResult) {
        guard case let .created(worktree, runSetup) = result else { return }
        appState.selectWorktree(projectID: project.id, worktree: worktree)
        guard runSetup,
              let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
        else { return }
        Task {
            await WorktreeSetupRunner.run(sourceProjectPath: project.path, paneID: paneID)
        }
    }

    private func removeWorktree(project: Project, worktree: Worktree) {
        let remaining = worktreeStore.list(for: project.id).filter { $0.id != worktree.id }
        let replacement = remaining.first(where: { $0.id == appState.activeWorktreeID[project.id] })
            ?? remaining.first(where: \.isPrimary)
            ?? remaining.first
        appState.removeWorktree(projectID: project.id, worktree: worktree, replacement: replacement)
        worktreeStore.remove(worktreeID: worktree.id, from: project.id)
    }
}

private struct BreadcrumbSegment: View {
    let symbol: String
    let text: String
    let busy: Bool
    let isOpen: Bool
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing2) {
                if busy {
                    ProgressView().controlSize(.mini)
                } else {
                    Image(systemName: symbol)
                        .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                }
                Text(text)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .monospaced))
                    .lineLimit(1)
                    .truncationMode(.middle)
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
            }
            .foregroundStyle(MuxyTheme.fgMuted)
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .background(background, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }

    private var background: AnyShapeStyle {
        if isOpen { return AnyShapeStyle(MuxyTheme.surface) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }
}
