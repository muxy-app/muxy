import AppKit
import SwiftUI

struct TabFocusedRepositoryToolbar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    @State private var repositoryState = TabFocusedRepositoryState()
    @State private var showBranchPopover = false
    @State private var showPullRequestPopover = false

    var body: some View {
        let context = repositoryContext
        return content(hasRepository: context != nil)
            .frame(height: UIMetrics.scaled(32))
            .task(id: context?.id ?? "no-repository") {
                guard let context else {
                    repositoryState.deactivate()
                    return
                }
                await repositoryState.activate(repoPath: context.path, context: context.workspaceContext)
            }
            .onDisappear {
                repositoryState.deactivate()
            }
            .onReceive(NotificationCenter.default.publisher(for: .vcsDidRefresh)) { notification in
                handleRepositoryNotification(notification)
            }
            .onReceive(NotificationCenter.default.publisher(for: .vcsRepoDidChange)) { notification in
                handleRepositoryNotification(notification)
            }
            .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
                Task { await repositoryState.refreshAfterAppActivation() }
            }
            .onChange(of: repositoryState.pullRequest) { _, pullRequest in
                if pullRequest == nil {
                    showPullRequestPopover = false
                }
            }
    }

    private func content(hasRepository: Bool) -> some View {
        HStack(spacing: UIMetrics.spacing3) {
            repositoryStatusContent(hasRepository: hasRepository)
            worktreeRemovalContent
        }
        .padding(.leading, UIMetrics.spacing6)
    }

    @ViewBuilder
    private func repositoryStatusContent(hasRepository: Bool) -> some View {
        switch RepositoryToolbarPresentation.contentState(
            hasRepository: hasRepository,
            hasSummary: repositoryState.summary != nil,
            error: repositoryState.summaryError
        ) {
        case .hidden:
            EmptyView()
        case .loading:
            repositoryLoadingView
        case let .unavailable(error):
            repositoryUnavailableChip(error)
        case .ready:
            if let summary = repositoryState.summary {
                repositoryContent(summary)
            }
        }
    }

    private func repositoryContent(_ summary: GitRepositorySummary) -> some View {
        HStack(spacing: UIMetrics.spacing3) {
            branchChip(summary)
            pullRequestContent
        }
    }

    private var repositoryLoadingView: some View {
        HStack(spacing: UIMetrics.spacing3) {
            ProgressView()
                .controlSize(.mini)
            Text("Reading repository…")
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Loading repository status")
    }

    private func repositoryUnavailableChip(_ error: String) -> some View {
        RepositoryToolbarChip(
            isOpen: false,
            action: {
                Task { await repositoryState.retryRepository() }
            },
            content: {
                HStack(spacing: UIMetrics.spacing2) {
                    if repositoryState.isLoadingSummary {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.system(size: UIMetrics.fontXS, weight: .bold))
                            .foregroundStyle(MuxyTheme.warning)
                    }
                    Text(repositoryState.isLoadingSummary ? "Reading repository…" : "Repository unavailable")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }
        )
        .disabled(repositoryState.isLoadingSummary)
        .help("\(error) Click to retry.")
        .accessibilityLabel("Repository unavailable. Click to retry.")
    }

    @ViewBuilder
    private var worktreeRemovalContent: some View {
        if let worktree = activeWorktree {
            let isPreparing = worktreeStore.isPreparingRemoval(worktreeID: worktree.id)
            let isRemoving = worktreeStore.isRemoving(worktreeID: worktree.id)
            let removalState = RepositoryToolbarPresentation.worktreeRemovalState(
                worktree: worktree,
                isPreparing: isPreparing,
                isRemoving: isRemoving
            )
            switch removalState {
            case .hidden:
                EmptyView()
            case .available:
                worktreeRemovalButton(worktree, state: removalState)
            case .preparing:
                worktreeRemovalButton(worktree, state: removalState)
            case .removing:
                worktreeRemovalButton(worktree, state: removalState)
            }
        }
    }

    private func worktreeRemovalButton(
        _ worktree: Worktree,
        state: RepositoryToolbarPresentation.WorktreeRemovalState
    ) -> some View {
        let isBusy = state != .available
        return RepositoryToolbarDestructiveButton(
            label: worktreeRemovalLabel(state),
            isBusy: isBusy,
            action: { requestWorktreeRemoval(worktree) }
        )
        .disabled(isBusy || repositoryState.isSwitchingBranch || isPerformingPullRequestAction)
        .help(worktreeRemovalHelp(worktree, state: state))
        .accessibilityLabel(worktreeRemovalHelp(worktree, state: state))
    }

    private func branchChip(_ summary: GitRepositorySummary) -> some View {
        RepositoryToolbarChip(
            isOpen: showBranchPopover,
            action: {
                showBranchPopover = true
                Task { await repositoryState.refreshRepositoryDetails() }
            },
            content: {
                HStack(spacing: UIMetrics.spacing2) {
                    if repositoryState.isSwitchingBranch {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: "arrow.triangle.branch")
                            .font(.system(size: UIMetrics.fontXS, weight: .bold))
                            .foregroundStyle(MuxyTheme.accent)
                    }
                    Text(summary.displayBranch)
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: UIMetrics.scaled(180))
                        .fixedSize(horizontal: true, vertical: false)
                    workingTreePulse(summary)
                    upstreamTelemetry(summary.aheadBehind)
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }
        )
        .disabled(isPerformingPullRequestAction || isWorktreeRemovalInProgress)
        .help(branchHelp(summary))
        .accessibilityLabel(branchHelp(summary))
        .popover(isPresented: $showBranchPopover, arrowEdge: .top) {
            TabFocusedBranchPopover(
                summary: repositoryState.summary ?? summary,
                branches: repositoryState.branches,
                isLoadingBranches: repositoryState.isLoadingBranches,
                isRefreshing: repositoryState.isLoadingSummary || repositoryState.isLoadingBranches,
                isSwitching: repositoryState.isSwitchingBranch,
                isWorktreeRemovalInProgress: isWorktreeRemovalInProgress,
                onSwitch: { branch in
                    switchBranch(branch)
                },
                onRefresh: {
                    Task { await repositoryState.refreshRepositoryDetails() }
                }
            )
        }
    }

    @ViewBuilder
    private var pullRequestContent: some View {
        switch repositoryState.pullRequestState {
        case .loading:
            ProgressView()
                .controlSize(.mini)
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                .help("Loading pull request")
        case .noPullRequest:
            EmptyView()
        case .unavailable:
            RepositoryToolbarChip(
                isOpen: false,
                action: {
                    Task { await repositoryState.refreshPullRequest(forceFresh: true) }
                },
                content: {
                    HStack(spacing: UIMetrics.spacing2) {
                        if repositoryState.isRefreshingPullRequest {
                            ProgressView().controlSize(.mini)
                        } else {
                            Image(systemName: "exclamationmark.triangle")
                                .font(.system(size: UIMetrics.fontXS, weight: .bold))
                        }
                        Text("PR unavailable")
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    }
                    .foregroundStyle(MuxyTheme.fgMuted)
                }
            )
            .disabled(
                repositoryState.isRefreshingPullRequest
                    || repositoryState.isSwitchingBranch
                    || isWorktreeRemovalInProgress
            )
            .help("Click to retry. GitHub pull requests require an installed and authenticated gh CLI.")
            .accessibilityLabel("Pull request unavailable. Retry GitHub connection.")
        case let .found(info):
            pullRequestChip(info)
        }
    }

    private func pullRequestChip(_ info: GitRepositoryService.PRInfo) -> some View {
        let color = PullRequestPresentation.color(for: info)
        return RepositoryToolbarChip(
            isOpen: showPullRequestPopover,
            action: { showPullRequestPopover = true },
            content: {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: PullRequestPresentation.symbol(for: info))
                        .font(.system(size: UIMetrics.fontXS, weight: .bold))
                        .foregroundStyle(color)
                    Text("PR #\(info.number)")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                    if let checks = pullRequestChecksChipLabel(info.checks) {
                        Text(checks)
                            .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .rounded))
                            .foregroundStyle(color)
                    }
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }
        )
        .disabled(repositoryState.isSwitchingBranch || isWorktreeRemovalInProgress)
        .help("Pull request #\(info.number) · \(PullRequestPresentation.stateLabel(for: info))")
        .accessibilityLabel("Pull request #\(info.number), \(PullRequestPresentation.stateLabel(for: info))")
        .popover(isPresented: $showPullRequestPopover, arrowEdge: .top) {
            if let context = pullRequestActionContext(for: repositoryState.pullRequest ?? info) {
                TabFocusedPullRequestPopover(
                    confirmationContext: context,
                    hasLocalChanges: repositoryState.summary?.isDirty ?? false,
                    isRefreshing: repositoryState.isRefreshingPullRequest,
                    isMerging: repositoryState.isMergingPullRequest,
                    isClosing: repositoryState.isClosingPullRequest,
                    isUpdatingBranch: repositoryState.isUpdatingPullRequestBranch,
                    isWorktreeRemovalInProgress: isWorktreeRemovalInProgress,
                    onMerge: { context, method in
                        performPullRequestAction(.merge(method), expected: context)
                    },
                    onClose: { context in
                        performPullRequestAction(.close, expected: context)
                    },
                    onOpenInBrowser: {
                        showPullRequestPopover = false
                        guard let url = URL(string: context.pullRequest.url) else { return }
                        NSWorkspace.shared.open(url)
                    },
                    onRefresh: {
                        Task { await repositoryState.refreshPullRequest(forceFresh: true) }
                    },
                    onUpdateBranch: {
                        updatePullRequestBranch(context.pullRequest)
                    }
                )
            }
        }
    }

    private func workingTreePulse(_ summary: GitRepositorySummary) -> some View {
        HStack(spacing: UIMetrics.spacing2) {
            Circle()
                .fill(summary.isDirty ? MuxyTheme.warning : MuxyTheme.diffAddFg)
                .frame(width: UIMetrics.scaled(5), height: UIMetrics.scaled(5))
            if summary.isDirty {
                Text("\(summary.changedCount)")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .rounded))
                    .foregroundStyle(MuxyTheme.warning)
            }
        }
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private func upstreamTelemetry(_ status: GitRepositoryService.AheadBehind) -> some View {
        if status.ahead > 0 || status.behind > 0 {
            HStack(spacing: UIMetrics.spacing2) {
                if status.ahead > 0 {
                    Text("↑\(status.ahead)")
                }
                if status.behind > 0 {
                    Text("↓\(status.behind)")
                }
            }
            .font(.system(size: UIMetrics.fontXS, weight: .semibold, design: .monospaced))
            .foregroundStyle(MuxyTheme.fgMuted)
            .accessibilityHidden(true)
        }
    }

    private var activeProject: Project? {
        guard let projectID = appState.activeProjectID,
              let project = projectStore.projects.first(where: { $0.id == projectID }),
              !project.isHome
        else { return nil }
        return project
    }

    private var activeWorktree: Worktree? {
        guard let activeProject else { return nil }
        return worktreeStore.preferred(
            for: activeProject.id,
            matching: appState.activeWorktreeID[activeProject.id]
        )
    }

    private var repositoryContext: RepositoryContext? {
        guard let project = activeProject else { return nil }
        let worktree = activeWorktree
        let path = worktree?.path ?? project.path
        return RepositoryContext(
            id: "\(project.id.uuidString)|\(worktree?.id.uuidString ?? "primary")|\(path)",
            path: path,
            workspaceContext: projectGroupStore.workspaceContext(for: project)
        )
    }

    private var isWorktreeRemovalInProgress: Bool {
        guard let activeWorktree else { return false }
        return worktreeStore.isRemovalInProgress(worktreeID: activeWorktree.id)
    }

    private func switchBranch(_ branch: String) {
        guard !isWorktreeRemovalInProgress, !isPerformingPullRequestAction else { return }
        showBranchPopover = false
        Task { await repositoryState.switchBranch(branch) }
    }

    private func updatePullRequestBranch(_ info: GitRepositoryService.PRInfo) {
        guard !isWorktreeRemovalInProgress,
              !repositoryState.isSwitchingBranch,
              !isPerformingPullRequestAction
        else { return }
        Task { await repositoryState.updatePullRequestBranch(info) }
    }

    private func requestWorktreeRemoval(_ worktree: Worktree) {
        guard !repositoryState.isSwitchingBranch,
              !isPerformingPullRequestAction,
              let currentWorktree = activeWorktree,
              currentWorktree.id == worktree.id,
              currentWorktree.canBeRemoved,
              !worktreeStore.isRemovalInProgress(worktreeID: currentWorktree.id)
        else { return }
        NotificationCenter.default.post(name: .removeCurrentWorktreeRequested, object: nil)
    }

    private func worktreeRemovalLabel(
        _ state: RepositoryToolbarPresentation.WorktreeRemovalState
    ) -> String {
        switch state {
        case .hidden,
             .available:
            "Remove worktree"
        case .preparing:
            "Checking…"
        case .removing:
            "Removing…"
        }
    }

    private func worktreeRemovalHelp(
        _ worktree: Worktree,
        state: RepositoryToolbarPresentation.WorktreeRemovalState
    ) -> String {
        switch state {
        case .hidden,
             .available:
            "Remove worktree \"\(worktree.name)\" and delete its files on disk"
        case .preparing:
            "Checking worktree \"\(worktree.name)\" for uncommitted changes"
        case .removing:
            "Removing worktree \"\(worktree.name)\""
        }
    }

    private func handleRepositoryNotification(_ notification: Notification) {
        guard repositoryState.shouldHandle(notification) else { return }
        Task { await repositoryState.refreshFromExternalChange() }
    }

    private func performPullRequestAction(
        _ action: PullRequestActionConfirmation.Kind,
        expected context: PullRequestActionConfirmation.Context
    ) {
        guard !repositoryState.isSwitchingBranch,
              !repositoryState.isRefreshingPullRequest,
              !isWorktreeRemovalInProgress,
              let currentPullRequest = repositoryState.pullRequest,
              pullRequestActionContext(for: currentPullRequest) == context
        else {
            ToastState.shared.show("Pull request context changed. Reopen the PR actions and try again.")
            return
        }
        let info = context.pullRequest
        switch action {
        case let .merge(method):
            let availability = PRMergeAvailability.make(info: info)
            guard availability.isEnabled else {
                ToastState.shared.show(title: "Pull request is no longer mergeable", body: availability.help)
                return
            }
            showPullRequestPopover = false
            Task { await repositoryState.mergePullRequest(info, method: method) }
        case .close:
            guard info.state == .open else {
                ToastState.shared.show("Pull request #\(info.number) is no longer open.")
                return
            }
            showPullRequestPopover = false
            Task { await repositoryState.closePullRequest(info) }
        }
    }

    private func pullRequestActionContext(
        for info: GitRepositoryService.PRInfo
    ) -> PullRequestActionConfirmation.Context? {
        guard let repositoryContext,
              let summary = repositoryState.summary,
              repositoryState.pullRequest == info
        else { return nil }
        return PullRequestActionConfirmation.Context(
            repositoryID: repositoryContext.id,
            branch: summary.branch,
            headOID: summary.headOID,
            pullRequest: info
        )
    }

    private var isPerformingPullRequestAction: Bool {
        repositoryState.isMergingPullRequest
            || repositoryState.isClosingPullRequest
            || repositoryState.isUpdatingPullRequestBranch
    }

    private func branchHelp(_ summary: GitRepositorySummary) -> String {
        let workingTree = workingTreeHelp(summary)
        let upstream = upstreamHelp(summary.aheadBehind)
        return "\(summary.displayBranch) · \(workingTree) · \(upstream)"
    }

    private func workingTreeHelp(_ summary: GitRepositorySummary) -> String {
        guard summary.isDirty else { return "Clean working tree" }
        return "\(summary.changedCount) changed, \(summary.stagedCount) staged, "
            + "\(summary.unstagedCount) unstaged, \(summary.untrackedCount) untracked"
    }

    private func upstreamHelp(_ status: GitRepositoryService.AheadBehind) -> String {
        guard status.hasUpstream else { return "No upstream" }
        guard status.ahead > 0 || status.behind > 0 else { return "Up to date" }
        return "\(status.ahead) ahead, \(status.behind) behind"
    }

    private func pullRequestChecksChipLabel(_ checks: GitRepositoryService.PRChecks) -> String? {
        switch checks.status {
        case .none: nil
        case .success: "\(checks.passing)/\(checks.total)"
        case .pending: "\(checks.pending) running"
        case .failure: "\(checks.failing) failing"
        }
    }
}

private struct RepositoryContext {
    let id: String
    let path: String
    let workspaceContext: WorkspaceContext
}

private struct RepositoryToolbarChip<Content: View>: View {
    let isOpen: Bool
    let action: () -> Void
    @ViewBuilder let content: () -> Content

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            content()
                .padding(.horizontal, UIMetrics.spacing3)
                .frame(height: UIMetrics.controlSmall)
                .background(background, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }

    private var background: Color {
        if isOpen { return MuxyTheme.surface }
        if hovered { return MuxyTheme.hover }
        return .clear
    }
}

private struct RepositoryToolbarDestructiveButton: View {
    let label: String
    let isBusy: Bool
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing2) {
                if isBusy {
                    ProgressView()
                        .controlSize(.mini)
                } else {
                    Image(systemName: "trash")
                        .font(.system(size: UIMetrics.fontXS, weight: .bold))
                }
                Text(label)
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            }
            .foregroundStyle(foreground)
            .padding(.horizontal, UIMetrics.spacing3)
            .frame(height: UIMetrics.controlSmall)
            .background(background, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }

    private var foreground: Color {
        if isBusy { return MuxyTheme.fgMuted }
        return hovered ? MuxyTheme.diffRemoveFg : MuxyTheme.fgMuted
    }

    private var background: Color {
        hovered && !isBusy ? MuxyTheme.hover : .clear
    }
}
