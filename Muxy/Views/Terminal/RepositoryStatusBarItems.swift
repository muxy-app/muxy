import AppKit
import SwiftUI

struct RepositoryStatusBarItems: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    @State private var repositoryState = TabFocusedRepositoryState()
    @State private var showBranchPopover = false
    @State private var showChangesPopover = false
    @State private var showPullRequestPopover = false
    @State private var installedProviderIDs: Set<String> = []
    @State private var aiActions = RepositoryAIActionsService.shared
    @State private var preparingPullRequestRepositoryID: String?
    @State private var pendingAIAction: RepositoryAIActionConfirmation?
    @AppStorage(RepositoryAIAction.commit.providerKey) private var commitProviderID = RepositoryAIActionPreferences.automaticProviderID
    @AppStorage(RepositoryAIAction.createPullRequest.providerKey) private var pullRequestProviderID = RepositoryAIActionPreferences
        .automaticProviderID

    var body: some View {
        let context = repositoryContext
        let confirmationContext = aiRepositoryActionContext(for: context)
        return content(hasRepository: context != nil)
            .frame(maxHeight: .infinity)
            .task(id: context?.id ?? "no-repository") {
                guard let context else {
                    repositoryState.deactivate()
                    return
                }
                async let providerRefresh: Void = refreshInstalledProviders()
                async let repositoryActivation: Void = repositoryState.activate(
                    repoPath: context.path,
                    context: context.workspaceContext
                )
                _ = await (providerRefresh, repositoryActivation)
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
                Task {
                    await refreshInstalledProviders()
                    await repositoryState.refreshAfterAppActivation()
                }
            }
            .onChange(of: repositoryState.pullRequest) { _, pullRequest in
                if pullRequest == nil {
                    showPullRequestPopover = false
                }
            }
            .onChange(of: isPerformingPullRequestAction) { wasPerforming, isPerforming in
                if wasPerforming, !isPerforming {
                    showPullRequestPopover = false
                }
            }
            .onChange(of: showChangesPopover) { _, isShowing in
                repositoryState.setChangesMonitoring(isShowing)
            }
            .onChange(of: confirmationContext) { _, _ in
                pendingAIAction = nil
            }
            .sheet(isPresented: aiRepositoryActionConfirmationBinding) {
                if let confirmation = pendingAIAction {
                    RepositoryAIActionConfirmationSheet(
                        confirmation: confirmation,
                        onCancel: {
                            pendingAIAction = nil
                        },
                        onConfirm: { additionalPrompt in
                            pendingAIAction = nil
                            confirmAIRepositoryAction(confirmation, additionalPrompt: additionalPrompt)
                        }
                    )
                }
            }
    }

    private var aiRepositoryActionConfirmationBinding: Binding<Bool> {
        Binding(
            get: { pendingAIAction != nil },
            set: { newValue in
                if !newValue {
                    pendingAIAction = nil
                }
            }
        )
    }

    @ViewBuilder
    private func content(hasRepository: Bool) -> some View {
        if hasRepository {
            HStack(spacing: UIMetrics.spacing4) {
                branchChip(repositoryState.summary)
                statusBarSeparator
                changesChip(repositoryState.summary)
                statusBarSeparator
                aiRepositoryAction(.commit, summary: repositoryState.summary, providerID: $commitProviderID)
                repositoryResultContent
            }
        }
    }

    @ViewBuilder
    private var repositoryResultContent: some View {
        if let summary = repositoryState.summary {
            switch repositoryState.pullRequestState {
            case .loading:
                EmptyView()
            case .noPullRequest:
                statusBarSeparator
                aiRepositoryAction(
                    .createPullRequest,
                    summary: summary,
                    providerID: $pullRequestProviderID
                )
            case .unavailable:
                statusBarSeparator
                pullRequestUnavailableChip
            case let .found(info):
                statusBarSeparator
                pullRequestChip(info)
            }
        } else if let error = repositoryState.summaryError {
            statusBarSeparator
            repositoryUnavailableChip(error)
        }
    }

    private var statusBarSeparator: some View {
        StatusBarSeparator()
    }

    private func repositoryUnavailableChip(_ error: String) -> some View {
        RepositoryToolbarChip(
            isOpen: false,
            action: {
                Task { await repositoryState.retryRepository() }
            },
            content: {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.system(size: UIMetrics.fontXS, weight: .bold))
                        .foregroundStyle(MuxyTheme.warning)
                    Text("Repository unavailable")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }
        )
        .disabled(repositoryState.isLoadingSummary)
        .help("\(error) Click to retry.")
        .accessibilityLabel("Repository unavailable. Click to retry.")
    }

    private func branchChip(_ summary: GitRepositorySummary?) -> some View {
        RepositoryToolbarChip(
            isOpen: showBranchPopover,
            action: {
                showBranchPopover = true
                Task { await repositoryState.refreshRepositoryDetails() }
            },
            content: {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: "arrow.triangle.branch")
                        .font(.system(size: UIMetrics.fontXS, weight: .bold))
                        .foregroundStyle(MuxyTheme.accent)
                    Text(RepositoryToolbarPresentation.branchLabel(summary: summary, worktree: activeWorktree))
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: UIMetrics.scaled(180))
                        .fixedSize(horizontal: true, vertical: false)
                    if let summary {
                        upstreamTelemetry(summary.aheadBehind)
                    }
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }
        )
        .disabled(
            summary == nil
                || repositoryState.isMutatingBranches
                || repositoryState.isMutatingChanges
                || isPerformingPullRequestAction
                || isWorktreeRemovalInProgress
                || hasRunningAIWorkflow
        )
        .help(summary.map(branchHelp) ?? "Loading repository status")
        .accessibilityLabel(summary.map(branchHelp) ?? "Loading repository status")
        .popover(isPresented: $showBranchPopover, arrowEdge: .bottom) {
            if let summary {
                TabFocusedBranchPopover(
                    summary: repositoryState.summary ?? summary,
                    branches: repositoryState.branches,
                    isLoadingBranches: repositoryState.isLoadingBranches,
                    isMutatingBranches: repositoryState.isMutatingBranches,
                    branchBeingDeleted: repositoryState.branchBeingDeleted,
                    isRepositoryInteractionDisabled: repositoryState.isMutatingChanges
                        || isPerformingPullRequestAction
                        || hasRunningAIWorkflow,
                    onSwitch: { branch in
                        switchBranch(branch)
                    },
                    onCreate: { branch in
                        await repositoryState.createAndSwitchBranch(branch)
                    },
                    onDelete: { branch in
                        await repositoryState.deleteBranch(branch)
                    }
                )
            }
        }
    }

    private func changesChip(_ summary: GitRepositorySummary?) -> some View {
        RepositoryToolbarChip(
            isOpen: showChangesPopover,
            action: {
                showChangesPopover = true
            },
            content: {
                HStack(spacing: UIMetrics.spacing2) {
                    Circle()
                        .fill(summary?.isDirty == true ? MuxyTheme.warning : MuxyTheme.diffAddFg)
                        .frame(width: UIMetrics.scaled(5), height: UIMetrics.scaled(5))
                    Text(summary.map(RepositoryChangesPresentation.chipLabel) ?? "Changes")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(summary?.isDirty == true ? MuxyTheme.warning : MuxyTheme.fgMuted)
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }
        )
        .disabled(
            summary == nil
                || repositoryState.isMutatingBranches
                || isPerformingPullRequestAction
                || repositoryState.isMutatingChanges
                || isWorktreeRemovalInProgress
                || hasRunningAIWorkflow
        )
        .help(summary.map(workingTreeHelp) ?? "Loading working tree status")
        .accessibilityLabel(summary.map(workingTreeHelp) ?? "Loading working tree status")
        .popover(isPresented: $showChangesPopover, arrowEdge: .bottom) {
            if let summary {
                TabFocusedChangesPopover(
                    summary: repositoryState.summary ?? summary,
                    changes: repositoryState.changesSnapshot,
                    untrackedLineStats: repositoryState.untrackedLineStats,
                    untrackedLineStatsSummary: repositoryState.untrackedLineStatsSummary,
                    hasLoadedChanges: repositoryState.hasLoadedChanges,
                    error: repositoryState.changesError,
                    isLoading: repositoryState.isLoadingChanges,
                    isMutating: repositoryState.isMutatingChanges,
                    isRepositoryInteractionDisabled: repositoryState.isMutatingBranches
                        || isPerformingPullRequestAction
                        || isWorktreeRemovalInProgress
                        || hasRunningAIWorkflow,
                    worktreeRemovalState: worktreeRemovalState,
                    worktreeRemovalHelp: activeWorktree.map {
                        worktreeRemovalHelp($0, state: worktreeRemovalState)
                    },
                    onRefresh: {
                        await repositoryState.refreshWorkingTreeDetails()
                    },
                    onStage: { file in
                        modifyChanges { await repositoryState.stage(file) }
                    },
                    onStageAll: {
                        let files = repositoryState.changesSnapshot.unstagedFiles
                        modifyChanges { await repositoryState.stage(files) }
                    },
                    onUnstage: { file in
                        modifyChanges { await repositoryState.unstage(file) }
                    },
                    onUnstageAll: {
                        let files = repositoryState.changesSnapshot.stagedFiles
                        modifyChanges { await repositoryState.unstage(files) }
                    },
                    onDiscard: { file in
                        modifyChanges { await repositoryState.discard(file) }
                    },
                    onLoadLineStats: { file in
                        await repositoryState.loadUntrackedLineStats(for: file)
                    },
                    onRemoveWorktree: {
                        showChangesPopover = false
                        guard let worktree = activeWorktree else { return }
                        requestWorktreeRemoval(worktree)
                    }
                )
            }
        }
    }

    private var pullRequestUnavailableChip: some View {
        RepositoryToolbarChip(
            isOpen: false,
            action: {
                Task { await repositoryState.refreshPullRequest(forceFresh: true) }
            },
            content: {
                Image(systemName: "arrow.triangle.pull")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
        )
        .disabled(
            repositoryState.isRefreshingPullRequest
                || repositoryState.isMutatingBranches
                || repositoryState.isMutatingChanges
                || isWorktreeRemovalInProgress
                || hasRunningAIWorkflow
        )
        .help("Click to retry. GitHub pull requests require an installed and authenticated gh CLI.")
        .accessibilityLabel("Pull request unavailable. Retry GitHub connection.")
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
        .disabled(
            repositoryState.isMutatingBranches
                || repositoryState.isMutatingChanges
                || isWorktreeRemovalInProgress
                || hasRunningAIWorkflow
        )
        .help("Pull request #\(info.number) · \(PullRequestPresentation.stateLabel(for: info))")
        .accessibilityLabel("Pull request #\(info.number), \(PullRequestPresentation.stateLabel(for: info))")
        .popover(isPresented: $showPullRequestPopover, arrowEdge: .bottom) {
            if let context = pullRequestActionContext(for: repositoryState.pullRequest ?? info) {
                TabFocusedPullRequestPopover(
                    confirmationContext: context,
                    hasLocalChanges: repositoryState.summary?.isDirty ?? false,
                    isRefreshing: repositoryState.isRefreshingPullRequest,
                    isMerging: repositoryState.isMergingPullRequest,
                    isClosing: repositoryState.isClosingPullRequest,
                    isUpdatingBranch: repositoryState.isUpdatingPullRequestBranch,
                    isWorktreeRemovalInProgress: isWorktreeRemovalInProgress
                        || repositoryState.isMutatingChanges,
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

    private func aiRepositoryAction(
        _ action: RepositoryAIAction,
        summary: GitRepositorySummary?,
        providerID: Binding<String>
    ) -> some View {
        let availability = aiRepositoryActionAvailability(action, summary: summary)
        return RepositoryAIActionSplitButton(
            action: action,
            providers: agentLaunchProviders,
            selectedProvider: selectedProvider(for: action),
            installedProviderIDs: installedProviderIDs,
            isRemote: repositoryContext?.workspaceContext.isRemote ?? false,
            availability: availability,
            isRunning: isRunningAIWorkflow(action),
            menuDisabled: hasRunningAIWorkflow,
            configuredProviderID: providerID,
            projectPrompt: projectPromptConfiguration(for: action),
            onRun: { requestAIRepositoryAction(action, availability: availability) }
        )
    }

    private func aiRepositoryActionAvailability(
        _ action: RepositoryAIAction,
        summary: GitRepositorySummary?
    ) -> RepositoryAIActionAvailability {
        switch action {
        case .commit:
            return RepositoryAIActionPresentation.commit(
                isDirty: summary?.isDirty,
                isDetached: summary?.isDetached,
                isRepositoryBusy: isRepositoryBusy,
                hasRunningAction: hasRunningAIWorkflow
            )
        case .createPullRequest:
            guard let summary else { return .hidden }
            return RepositoryAIActionPresentation.createPullRequest(
                pullRequest: pullRequestPresence,
                isDirty: summary.isDirty,
                isDetached: summary.isDetached,
                isRepositoryBusy: isRepositoryBusy,
                hasRunningAction: hasRunningAIWorkflow
            )
        }
    }

    private var pullRequestPresence: RepositoryPullRequestPresence {
        switch repositoryState.pullRequestState {
        case .loading: .loading
        case .noPullRequest: .none
        case .unavailable: .unavailable
        case .found: .found
        }
    }

    private var agentLaunchProviders: [any AIAgentLaunchProvider] {
        AIProviderRegistry.shared.agentLaunchProviders
    }

    private func projectPromptConfiguration(
        for action: RepositoryAIAction
    ) -> RepositoryAIActionProjectPromptConfiguration? {
        guard action == .createPullRequest, let project = activeProject else { return nil }
        return RepositoryAIActionProjectPromptConfiguration(
            projectName: project.name,
            prompt: project.pullRequestPrompt,
            fallbackPrompt: RepositoryAIActionPreferences.prompt(for: action),
            onSave: { prompt in
                projectStore.setPullRequestPrompt(id: project.id, to: prompt)
            }
        )
    }

    private func selectedProvider(
        for action: RepositoryAIAction
    ) -> (any AIAgentLaunchProvider)? {
        RepositoryAIActionsService.resolveProvider(
            for: action,
            providers: agentLaunchProviders,
            installedProviderIDs: installedProviderIDs,
            isRemote: repositoryContext?.workspaceContext.isRemote ?? false
        )
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
        guard let project = activeProject,
              let worktree = activeWorktree
        else { return nil }
        let path = worktree.path
        return RepositoryContext(
            id: "\(project.id.uuidString)|\(worktree.id.uuidString)|\(path)",
            path: path,
            workspaceContext: projectGroupStore.workspaceContext(for: project)
        )
    }

    private var isWorktreeRemovalInProgress: Bool {
        guard let activeWorktree else { return false }
        return worktreeStore.isRemovalInProgress(worktreeID: activeWorktree.id)
    }

    private var worktreeRemovalState: RepositoryToolbarPresentation.WorktreeRemovalState {
        guard let activeWorktree else { return .hidden }
        return RepositoryToolbarPresentation.worktreeRemovalState(
            worktree: activeWorktree,
            isPreparing: worktreeStore.isPreparingRemoval(worktreeID: activeWorktree.id),
            isRemoving: worktreeStore.isRemoving(worktreeID: activeWorktree.id)
        )
    }

    private func switchBranch(_ branch: String) {
        guard !isWorktreeRemovalInProgress,
              !repositoryState.isMutatingBranches,
              !repositoryState.isMutatingChanges,
              !isPerformingPullRequestAction,
              !hasRunningAIWorkflow
        else { return }
        showBranchPopover = false
        Task { await repositoryState.switchBranch(branch) }
    }

    private func updatePullRequestBranch(_ info: GitRepositoryService.PRInfo) {
        guard !isWorktreeRemovalInProgress,
              !repositoryState.isMutatingBranches,
              !repositoryState.isMutatingChanges,
              !isPerformingPullRequestAction,
              !hasRunningAIWorkflow
        else { return }
        Task { await repositoryState.updatePullRequestBranch(info) }
    }

    private func requestWorktreeRemoval(_ worktree: Worktree) {
        guard !repositoryState.isMutatingBranches,
              !repositoryState.isMutatingChanges,
              !isPerformingPullRequestAction,
              !hasRunningAIWorkflow,
              let currentWorktree = activeWorktree,
              currentWorktree.id == worktree.id,
              currentWorktree.canBeRemoved,
              !worktreeStore.isRemovalInProgress(worktreeID: currentWorktree.id)
        else { return }
        NotificationCenter.default.post(name: .removeCurrentWorktreeRequested, object: nil)
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
        guard !repositoryState.isMutatingBranches,
              !repositoryState.isMutatingChanges,
              !repositoryState.isRefreshingPullRequest,
              !isWorktreeRemovalInProgress,
              !hasRunningAIWorkflow,
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
            Task { await repositoryState.mergePullRequest(info, method: method) }
        case .close:
            guard info.state == .open else {
                ToastState.shared.show("Pull request #\(info.number) is no longer open.")
                return
            }
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

    private var isRepositoryBusy: Bool {
        repositoryState.isMutatingBranches
            || repositoryState.isMutatingChanges
            || isPerformingPullRequestAction
            || isWorktreeRemovalInProgress
    }

    private var hasRunningAIWorkflow: Bool {
        guard let repositoryID = repositoryContext?.id else { return false }
        return preparingPullRequestRepositoryID == repositoryID || aiActions.isRunning(repositoryID: repositoryID)
    }

    private func isRunningAIWorkflow(_ action: RepositoryAIAction) -> Bool {
        guard let repositoryID = repositoryContext?.id else { return false }
        return (action == .createPullRequest && preparingPullRequestRepositoryID == repositoryID)
            || aiActions.isRunning(repositoryID: repositoryID, action: action)
    }

    private func refreshInstalledProviders() async {
        await LoginShellPath.hydrate()
        installedProviderIDs = Set(agentLaunchProviders.filter { $0.isAgentCLIInstalled() }.map(\.id))
    }

    private func aiRepositoryActionContext(
        for context: RepositoryContext?
    ) -> RepositoryAIActionConfirmation.Context? {
        guard let context,
              let summary = repositoryState.summary
        else { return nil }
        return RepositoryAIActionConfirmation.Context(
            repositoryID: context.id,
            branch: summary.branch
        )
    }

    private func requestAIRepositoryAction(
        _ action: RepositoryAIAction,
        availability: RepositoryAIActionAvailability
    ) {
        guard availability == .available,
              !hasRunningAIWorkflow,
              let context = aiRepositoryActionContext(for: repositoryContext),
              let provider = selectedProvider(for: action)
        else { return }
        pendingAIAction = RepositoryAIActionConfirmation(
            action: action,
            context: context,
            providerName: provider.displayName
        )
    }

    private func confirmAIRepositoryAction(
        _ confirmation: RepositoryAIActionConfirmation,
        additionalPrompt: String?
    ) {
        let action = confirmation.action
        let availability = aiRepositoryActionAvailability(action, summary: repositoryState.summary)
        guard availability == .available,
              !hasRunningAIWorkflow,
              aiRepositoryActionContext(for: repositoryContext) == confirmation.context
        else {
            ToastState.shared.show("\(action.settingsTitle) is no longer available. Try again.")
            return
        }
        let instructions = RepositoryAIActionPreferences.instructions(
            for: action,
            projectPrompt: action == .createPullRequest ? activeProject?.pullRequestPrompt : nil,
            additionalPrompt: additionalPrompt
        )
        runAIRepositoryAction(action, availability: availability, instructions: instructions)
    }

    private func runAIRepositoryAction(
        _ action: RepositoryAIAction,
        availability: RepositoryAIActionAvailability,
        instructions: String
    ) {
        guard availability == .available,
              !hasRunningAIWorkflow,
              let context = repositoryContext
        else { return }

        if action == .createPullRequest {
            preparingPullRequestRepositoryID = context.id
            Task {
                defer {
                    if preparingPullRequestRepositoryID == context.id {
                        preparingPullRequestRepositoryID = nil
                    }
                }
                await repositoryState.refreshPullRequest(forceFresh: true)
                guard repositoryContext?.id == context.id
                else { return }
                guard pullRequestPresence == .none else {
                    if pullRequestPresence == .unavailable {
                        ToastState.shared.show("Could not verify that this branch has no pull request.")
                    }
                    return
                }
                startAIRepositoryAction(action, context: context, instructions: instructions)
            }
            return
        }

        startAIRepositoryAction(action, context: context, instructions: instructions)
    }

    private func startAIRepositoryAction(
        _ action: RepositoryAIAction,
        context: RepositoryContext,
        instructions: String
    ) {
        guard let summary = repositoryState.summary else { return }
        let serviceContext = RepositoryAIActionsService.Context(
            repositoryID: context.id,
            path: context.path,
            workspaceContext: context.workspaceContext,
            expectedBranch: summary.branch,
            hasUpstream: summary.aheadBehind.hasUpstream
        )

        do {
            try aiActions.start(
                action: action,
                context: serviceContext,
                providers: agentLaunchProviders,
                installedProviderIDs: installedProviderIDs,
                instructions: instructions
            )
        } catch {
            ToastState.shared.show(title: "Could not start \(action.settingsTitle)", body: error.localizedDescription)
        }
    }

    private func branchHelp(_ summary: GitRepositorySummary) -> String {
        let upstream = upstreamHelp(summary.aheadBehind)
        return "\(summary.displayBranch) · \(upstream)"
    }

    private func modifyChanges(_ operation: @escaping @MainActor () async -> Void) {
        guard !isRepositoryBusy, !hasRunningAIWorkflow else { return }
        Task { await operation() }
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
