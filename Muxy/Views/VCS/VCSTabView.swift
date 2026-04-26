import AppKit
import SwiftUI

struct VCSTabView: View {
    @Bindable var state: VCSTabState
    let focused: Bool
    let onFocus: () -> Void
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var showDiscardAllConfirmation = false
    @State private var pendingDiscardPath: String?
    @State private var showCreateWorktreeSheet = false
    @State private var showCreateBranchSheet = false
    @State private var showInlinePRForm = false
    @State private var pendingClosePR: GitRepositoryService.PRInfo?
    @State private var pendingCheckoutPR: GitRepositoryService.PRListItem?
    private var commitEnabled: Bool {
        state.hasStagedChanges && !state.commitMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var owningProject: Project? {
        if let id = worktreeStore.projectID(forWorktreePath: state.projectPath) {
            return projectStore.projects.first { $0.id == id }
        }
        return projectStore.projects.first { $0.path == state.projectPath }
    }

    private var activeWorktreeForTab: Worktree? {
        guard let project = owningProject else { return nil }
        return worktreeStore.list(for: project.id).first { $0.path == state.projectPath }
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            content
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .onTapGesture(perform: onFocus)
        .onAppear {
            if !state.hasCompletedInitialLoad, !state.isLoadingFiles {
                state.refresh()
            }
        }
        .onChange(of: state.projectPath) {
            if !state.hasCompletedInitialLoad, !state.isLoadingFiles {
                state.refresh()
            }
        }
        .onChange(of: state.showPushUpstreamConfirmation) { _, show in
            guard show else { return }
            state.showPushUpstreamConfirmation = false
            presentPushUpstreamConfirmation()
        }
        .onChange(of: showDiscardAllConfirmation) { _, show in
            guard show else { return }
            showDiscardAllConfirmation = false
            presentDiscardConfirmation(
                title: "Discard All Changes?",
                message: "This will discard all uncommitted changes. This cannot be undone.",
                buttonTitle: "Discard All"
            ) {
                state.discardAll()
            }
        }
        .onChange(of: pendingDiscardPath) { _, path in
            guard let path else { return }
            pendingDiscardPath = nil
            let fileName = (path as NSString).lastPathComponent
            presentDiscardConfirmation(
                title: "Discard Changes?",
                message: "Discard changes to \(fileName)?",
                buttonTitle: "Discard"
            ) {
                state.discardFile(path)
            }
        }
        .onChange(of: pendingClosePR?.number) { _, number in
            guard number != nil, let prInfo = pendingClosePR else { return }
            pendingClosePR = nil
            presentClosePRConfirmation(prInfo: prInfo)
        }
        .onChange(of: pendingCheckoutPR?.number) { _, number in
            guard number != nil, let pr = pendingCheckoutPR else { return }
            pendingCheckoutPR = nil
            presentCheckoutPRConfirmation(pr: pr)
        }
        .alert(
            "Error",
            isPresented: Binding(
                get: { state.statusIsError && state.statusMessage != nil },
                set: { if !$0 { state.statusMessage = nil } }
            )
        ) {
            Button("OK", role: .cancel) { state.statusMessage = nil }
        } message: {
            if let message = state.statusMessage {
                Text(message)
            }
        }
    }

    private var header: some View {
        HStack(spacing: 6) {
            worktreeBranchPicker

            PRPill(
                state: state,
                onRequestCreate: { requestOpenPR() },
                onRequestMerge: { prInfo, method in performMerge(prInfo: prInfo, method: method) },
                onRequestClose: { prInfo in pendingClosePR = prInfo }
            )

            Spacer(minLength: 0)

            if let url = state.remoteWebURL {
                IconButton(symbol: "globe", accessibilityLabel: "Open Repository on Web") {
                    NSWorkspace.shared.open(url)
                }
                .help("Open repository on web")
            }

            VCSSectionVisibilityMenu(state: state)

            if state.isRefreshingPullRequest {
                ProgressView()
                    .controlSize(.small)
                    .frame(width: 24, height: 24)
            } else {
                IconButton(symbol: "arrow.clockwise", accessibilityLabel: "Refresh") {
                    state.refresh()
                }
            }
        }
        .padding(.horizontal, 8)
        .frame(height: 32)
        .background(MuxyTheme.bg)
        .sheet(isPresented: $showCreateWorktreeSheet) {
            if let project = owningProject {
                CreateWorktreeSheet(project: project) { result in
                    showCreateWorktreeSheet = false
                    handleCreateWorktreeResult(result, project: project)
                }
            }
        }
        .sheet(isPresented: $showCreateBranchSheet) {
            CreateBranchSheet(
                currentBranch: state.branchName,
                onCreate: { name in
                    showCreateBranchSheet = false
                    state.createAndSwitchBranch(name)
                },
                onCancel: { showCreateBranchSheet = false }
            )
        }
        .onChange(of: state.pullRequestInfo?.number) { _, number in
            guard number != nil, showInlinePRForm else { return }
            showInlinePRForm = false
        }
    }

    private func requestOpenPR() {
        state.openPullRequestError = nil
        state.loadBranches()
        showInlinePRForm = true
    }

    @ViewBuilder
    private var worktreeBranchPicker: some View {
        if let project = owningProject {
            WorktreeBranchPicker(
                project: project,
                isGitRepo: state.isGitRepo,
                currentBranch: state.branchName,
                branches: state.branches,
                isLoadingBranches: state.isLoadingBranches,
                activeWorktree: activeWorktreeForTab,
                onSelectBranch: { state.switchBranch($0) },
                onRefreshBranches: { state.loadBranches() },
                onCreateBranch: { showCreateBranchSheet = true },
                onDeleteBranch: { branch in presentDeleteBranchConfirmation(branch) },
                onRequestCreateWorktree: { showCreateWorktreeSheet = true }
            )
            .environment(appState)
            .environment(worktreeStore)
        }
    }

    private func performMerge(prInfo: GitRepositoryService.PRInfo, method: GitRepositoryService.PRMergeMethod) {
        if prInfo.checks.status == .failure || prInfo.checks.status == .pending {
            presentChecksMergeConfirmation(prInfo: prInfo, method: method)
            return
        }
        continueMergeAfterChecks(prInfo: prInfo, method: method)
    }

    private func continueMergeAfterChecks(prInfo: GitRepositoryService.PRInfo, method: GitRepositoryService.PRMergeMethod) {
        if state.hasAnyChanges {
            presentDirtyMergeConfirmation(prInfo: prInfo, method: method)
            return
        }
        executeMerge(prInfo: prInfo, method: method)
    }

    private func presentChecksMergeConfirmation(prInfo: GitRepositoryService.PRInfo, method: GitRepositoryService.PRMergeMethod) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let isFailure = prInfo.checks.status == .failure
        let messageText = isFailure
            ? "Merge PR #\(prInfo.number) with failing checks?"
            : "Merge PR #\(prInfo.number) while checks are still running?"
        let informativeText = isFailure
            ? "\(prInfo.checks.failing) check(s) are failing. Merging now may introduce broken code into the base branch."
            : "\(prInfo.checks.pending) check(s) are still running. Merging now will bypass them."

        let alert = NSAlert()
        alert.messageText = messageText
        alert.informativeText = informativeText
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Merge Anyway")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = ""
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            guard response == .alertFirstButtonReturn else { return }
            continueMergeAfterChecks(prInfo: prInfo, method: method)
        }
    }

    private func executeMerge(prInfo: GitRepositoryService.PRInfo, method: GitRepositoryService.PRMergeMethod) {
        let project = owningProject
        let worktree = activeWorktreeForTab
        let defaultBranch = state.defaultBranch
        let isWorktreeMerge = worktree.map { !$0.isPrimary } ?? false
        state.mergePullRequest(method: method, deleteBranch: !isWorktreeMerge) { _, mergedBranch in
            ToastState.shared.show("Merged PR #\(prInfo.number)")
            Task { @MainActor in
                await cleanupAfterMerge(
                    mergedBranch: mergedBranch,
                    project: project,
                    worktree: worktree,
                    defaultBranch: defaultBranch
                )
            }
        }
    }

    private func presentDirtyMergeConfirmation(prInfo: GitRepositoryService.PRInfo, method: GitRepositoryService.PRMergeMethod) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let worktree = activeWorktreeForTab
        let willDiscard = worktree.map { !$0.isPrimary } ?? false

        let worktreeWarning = """
        You have uncommitted changes in this worktree. After the merge, the worktree will be \
        removed and those changes will be lost permanently.
        """
        let branchWarning = """
        You have uncommitted changes on this branch. After the merge, this branch will be \
        deleted on the remote and those changes will no longer belong to any branch.
        """

        let alert = NSAlert()
        alert.messageText = "Merge PR #\(prInfo.number) with uncommitted changes?"
        alert.informativeText = willDiscard ? worktreeWarning : branchWarning
        alert.alertStyle = .critical
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Merge Anyway")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = ""
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            guard response == .alertFirstButtonReturn else { return }
            executeMerge(prInfo: prInfo, method: method)
        }
    }

    private func cleanupAfterMerge(
        mergedBranch: String,
        project: Project?,
        worktree: Worktree?,
        defaultBranch: String?
    ) async {
        if let project, let worktree, worktree.canBeRemoved {
            removeWorktreeAfterMerge(project: project, worktree: worktree, mergedBranch: mergedBranch)
            return
        }

        if let defaultBranch, defaultBranch != mergedBranch {
            await state.switchBranchAndRefresh(defaultBranch)
        }
    }

    private func removeWorktreeAfterMerge(project: Project, worktree: Worktree, mergedBranch: String) {
        let repoPath = project.path
        let remaining = worktreeStore.list(for: project.id).filter { $0.id != worktree.id }
        let replacement = remaining.first(where: { $0.isPrimary }) ?? remaining.first
        appState.removeWorktree(
            projectID: project.id,
            worktree: worktree,
            replacement: replacement
        )
        worktreeStore.remove(worktreeID: worktree.id, from: project.id)
        Task.detached {
            await WorktreeStore.cleanupOnDisk(
                worktree: worktree,
                repoPath: repoPath
            )
            try? await GitRepositoryService().deleteRemoteBranch(
                repoPath: repoPath,
                branch: mergedBranch
            )
        }
    }

    private func presentClosePRConfirmation(prInfo: GitRepositoryService.PRInfo) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = "Close PR #\(prInfo.number)?"
        alert.informativeText = "This will close the pull request on GitHub without merging. You can reopen it later."
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Close PR")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = "\r"
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            guard response == .alertFirstButtonReturn else { return }
            state.closePullRequest {}
        }
    }

    private func handleCreateWorktreeResult(_ result: CreateWorktreeResult, project: Project) {
        switch result {
        case let .created(worktree, runSetup):
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            if runSetup,
               let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
            {
                Task {
                    await WorktreeSetupRunner.run(
                        sourceProjectPath: project.path,
                        paneID: paneID
                    )
                }
            }
        case .cancelled:
            break
        }
    }

    @ViewBuilder
    private var content: some View {
        if state.isLoadingFiles {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if state.files.isEmpty, state.errorMessage != nil {
            Text(state.errorMessage ?? "")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            VStack(spacing: 0) {
                if showInlinePRForm {
                    createPRForm
                } else {
                    commitArea
                }
                SectionSplitLayout(
                    state: state,
                    onFocus: onFocus,
                    showDiscardAllConfirmation: $showDiscardAllConfirmation,
                    pendingDiscardPath: $pendingDiscardPath,
                    pendingCheckoutPR: $pendingCheckoutPR,
                    onOpenInEditor: openFileInEditor,
                    onOpenDiff: openDiffInTab
                )
            }
        }
    }

    private var createPRForm: some View {
        CreatePRForm(
            context: CreatePRForm.Context(
                currentBranch: state.branchName ?? "",
                defaultBranch: state.defaultBranch,
                localBranches: state.branches,
                remoteBranches: state.remoteBranches,
                isLoadingRemoteBranches: state.isLoadingRemoteBranches,
                hasStagedChanges: state.hasStagedChanges,
                hasUnstagedChanges: !state.unstagedFiles.isEmpty
            ),
            inProgress: state.isOpeningPullRequest,
            errorMessage: state.openPullRequestError,
            onLoadRemoteBranches: { state.loadRemoteBranches() },
            onSubmit: { base, title, body, branchStrategy, includeMode, draft in
                ToastState.shared.show("Creating pull request…")
                state.openPullRequest(
                    VCSTabState.PRCreateRequest(
                        baseBranch: base,
                        title: title,
                        body: body,
                        branchStrategy: branchStrategy,
                        includeMode: includeMode,
                        draft: draft
                    )
                )
            },
            onCancel: {
                state.openPullRequestError = nil
                showInlinePRForm = false
            }
        )
    }

    private var commitArea: some View {
        VStack(spacing: 8) {
            ZStack(alignment: .topLeading) {
                if state.commitMessage.isEmpty {
                    Text("Commit message (⌘↵ to commit on \(state.branchName ?? "branch"))")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 10)
                        .allowsHitTesting(false)
                }
                TextEditor(text: $state.commitMessage)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
                    .scrollContentBackground(.hidden)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 9)
                    .frame(minHeight: 27, maxHeight: 50)
                    .onKeyPress(.return, phases: .down) { keyPress in
                        if keyPress.modifiers.contains(.command) {
                            state.commit()
                            return .handled
                        }
                        return .ignored
                    }
            }
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(MuxyTheme.border, lineWidth: 1))

            HStack(spacing: 6) {
                commitButton
                pullButton
                pushButton
            }
        }
        .padding(10)
        .background(MuxyTheme.bg)
    }

    private var commitButton: some View {
        Button {
            state.commit()
        } label: {
            HStack(spacing: 4) {
                if state.isCommitting {
                    ProgressView().controlSize(.mini)
                } else {
                    Image(systemName: "checkmark")
                        .font(.system(size: 10, weight: .bold))
                }
                Text("Commit")
                    .font(.system(size: 11, weight: .medium))
            }
            .foregroundStyle(commitEnabled ? MuxyTheme.bg : MuxyTheme.fgDim)
            .frame(maxWidth: .infinity)
            .frame(height: Self.actionButtonHeight)
            .background(
                commitEnabled ? MuxyTheme.accent : MuxyTheme.surface,
                in: RoundedRectangle(cornerRadius: 6)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(MuxyTheme.border, lineWidth: commitEnabled ? 0 : 1)
            )
        }
        .buttonStyle(.plain)
        .disabled(!commitEnabled || state.isCommitting)
        .help("Commit staged changes")
    }

    private var pullButton: some View {
        Button {
            state.pull()
        } label: {
            HStack(spacing: 4) {
                if state.isPulling {
                    ProgressView().controlSize(.mini)
                } else {
                    Image(systemName: "arrow.down")
                        .font(.system(size: 10, weight: .bold))
                }
                Text("Pull")
                    .font(.system(size: 11, weight: .medium))
                if state.aheadBehind.behind > 0 {
                    Text("\(state.aheadBehind.behind)")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.bg)
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(MuxyTheme.diffAddFg, in: Capsule())
                }
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, 10)
            .frame(height: Self.actionButtonHeight)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(MuxyTheme.border, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .disabled(state.isPulling)
        .help(state.aheadBehind.behind > 0
            ? "Pull \(state.aheadBehind.behind) commit\(state.aheadBehind.behind == 1 ? "" : "s") from origin"
            : "Pull from origin")
    }

    private var pushButton: some View {
        Button {
            state.push()
        } label: {
            HStack(spacing: 4) {
                if state.isPushing {
                    ProgressView().controlSize(.mini)
                } else {
                    Image(systemName: "arrow.up")
                        .font(.system(size: 10, weight: .bold))
                }
                Text("Push")
                    .font(.system(size: 11, weight: .medium))
                if state.aheadBehind.ahead > 0 {
                    Text("\(state.aheadBehind.ahead)")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.bg)
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(MuxyTheme.accent, in: Capsule())
                }
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, 10)
            .frame(height: Self.actionButtonHeight)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(MuxyTheme.border, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .disabled(state.isPushing)
        .help(state.aheadBehind.ahead > 0
            ? "Push \(state.aheadBehind.ahead) commit\(state.aheadBehind.ahead == 1 ? "" : "s") to origin"
            : "Push to origin")
    }

    private static let actionButtonHeight: CGFloat = 28

    private func presentDiscardConfirmation(
        title: String,
        message: String,
        buttonTitle: String,
        onConfirm: @escaping () -> Void
    ) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: buttonTitle)
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = "\r"
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            if response == .alertFirstButtonReturn {
                onConfirm()
            }
        }
    }

    private func presentDeleteBranchConfirmation(_ branch: String) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = "Delete Branch?"
        alert.informativeText = "This will permanently delete the local branch \"\(branch)\". Unmerged commits on this branch will be lost."
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Delete")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = "\r"
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            guard response == .alertFirstButtonReturn else { return }
            Task { await state.deleteLocalBranch(branch) }
        }
    }

    private func presentPushUpstreamConfirmation() {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let branch = state.branchName ?? "current branch"
        let alert = NSAlert()
        alert.messageText = "Push to Remote?"
        alert.informativeText = "The branch \"\(branch)\" has no upstream on the remote. Push and set upstream to origin/\(branch)?"
        alert.alertStyle = .informational
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Push")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = "\r"
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            if response == .alertFirstButtonReturn {
                state.pushSetUpstream()
            }
        }
    }

    private func presentCheckoutPRConfirmation(pr: GitRepositoryService.PRListItem) {
        let isDirty = !state.files.isEmpty
        guard isDirty else {
            state.checkoutPullRequest(pr)
            return
        }
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = "Checkout PR #\(pr.number)?"
        alert.informativeText = "You have uncommitted changes. Switching branches may fail or move them. Continue?"
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Checkout")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = "\r"
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            if response == .alertFirstButtonReturn {
                state.checkoutPullRequest(pr)
            }
        }
    }

    private func openFileInEditor(_ relativePath: String) {
        guard let projectID = appState.activeProjectID else { return }
        let fullPath = state.projectPath.hasSuffix("/")
            ? state.projectPath + relativePath
            : state.projectPath + "/" + relativePath
        appState.openFile(fullPath, projectID: projectID)
    }

    private func openDiffInTab(_ relativePath: String, isStaged: Bool) {
        guard let projectID = appState.activeProjectID else { return }
        appState.openDiffViewer(vcs: state, filePath: relativePath, isStaged: isStaged, projectID: projectID)
    }
}

struct VCSSectionVisibilityMenu: View {
    @Bindable var state: VCSTabState
    @State private var hovered = false

    private struct Row: Identifiable {
        let id: String
        let visible: Bool
        let toggle: () -> Void
        var title: String { id }
    }

    private var rows: [Row] {
        [
            Row(id: "Changes", visible: state.changesVisible) { state.setChangesVisible(!state.changesVisible) },
            Row(id: "Pull Requests", visible: state.pullRequestsVisible) { state.setPullRequestsVisible(!state.pullRequestsVisible) },
            Row(id: "History", visible: state.historyVisible) { state.setHistoryVisible(!state.historyVisible) },
        ]
    }

    var body: some View {
        Menu {
            ForEach(rows) { row in
                Button(action: row.toggle) {
                    if row.visible {
                        Label(row.title, systemImage: "checkmark")
                    } else {
                        Text(row.title)
                    }
                }
            }
        } label: {
            Image(systemName: "sidebar.squares.left")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: 24, height: 24)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .onHover { hovered = $0 }
        .accessibilityLabel("Show/Hide Sections")
        .help("Show/Hide sections")
    }
}

struct PRPill: View {
    @Bindable var state: VCSTabState
    let onRequestCreate: () -> Void
    let onRequestMerge: (GitRepositoryService.PRInfo, GitRepositoryService.PRMergeMethod) -> Void
    let onRequestClose: (GitRepositoryService.PRInfo) -> Void

    @State private var showPRPopover = false

    var body: some View {
        if !state.hasFetchedPullRequestInfo {
            EmptyView()
        } else {
            switch state.prLaunchState {
            case .hidden:
                EmptyView()
            case .ghMissing:
                ghMissingPill
            case .canCreate:
                createPRPill
            case let .hasPR(info):
                hasPRPill(info: info)
            }
        }
    }

    private var ghMissingPill: some View {
        pillContainer(
            icon: "exclamationmark.triangle",
            text: "Install gh",
            tint: MuxyTheme.fgMuted,
            disabled: true
        ) {}
            .help("Install GitHub CLI to create pull requests: brew install gh")
    }

    private var createPRPill: some View {
        Button(action: onRequestCreate) {
            HStack(spacing: 4) {
                Image(systemName: "arrow.triangle.pull")
                    .font(.system(size: 9, weight: .bold))
                Text("Create PR")
                    .font(.system(size: 10, weight: .semibold))
            }
            .foregroundStyle(MuxyTheme.accent)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(state.isOpeningPullRequest)
        .help("Create a pull request")
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
        .overlay(RoundedRectangle(cornerRadius: 5).stroke(MuxyTheme.accent.opacity(0.35), lineWidth: 1))
    }

    private func hasPRPill(info: GitRepositoryService.PRInfo) -> some View {
        Button {
            showPRPopover = true
        } label: {
            HStack(spacing: 4) {
                Image(systemName: prStateIcon(info))
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(prStateColor(info))
                Text("PR #\(info.number)")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg.opacity(0.85))
                Image(systemName: "chevron.down")
                    .font(.system(size: 8, weight: .bold))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
            .overlay(RoundedRectangle(cornerRadius: 5).stroke(prStateColor(info).opacity(0.35), lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: 5))
        }
        .buttonStyle(.plain)
        .help("Pull request #\(info.number)")
        .popover(isPresented: $showPRPopover, arrowEdge: .top) {
            PRPopover(
                state: state,
                info: info,
                onMerge: { method in
                    let needsConfirmation = state.hasAnyChanges
                        || info.checks.status == .failure
                        || info.checks.status == .pending
                    if needsConfirmation {
                        showPRPopover = false
                    }
                    onRequestMerge(info, method)
                },
                onClose: {
                    showPRPopover = false
                    onRequestClose(info)
                },
                onOpenInBrowser: {
                    showPRPopover = false
                    if let url = URL(string: info.url) {
                        NSWorkspace.shared.open(url)
                    }
                },
                onRefresh: {
                    state.refreshPullRequest()
                }
            )
        }
        .onChange(of: state.pullRequestInfo?.number) { _, number in
            if number == nil, showPRPopover {
                showPRPopover = false
            }
        }
    }

    private func prStateIcon(_ info: GitRepositoryService.PRInfo) -> String {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return "xmark.octagon.fill"
            case .pending: return "clock"
            default: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? "pencil.circle" : "arrow.triangle.pull"
        case .merged: return "checkmark.circle.fill"
        case .closed: return "xmark.circle"
        }
    }

    private func prStateColor(_ info: GitRepositoryService.PRInfo) -> Color {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return MuxyTheme.diffRemoveFg
            case .pending: return MuxyTheme.fgMuted
            default: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? MuxyTheme.fgMuted : MuxyTheme.diffAddFg
        case .merged: return MuxyTheme.accent
        case .closed: return MuxyTheme.diffRemoveFg
        }
    }

    private func pillContainer(
        icon: String,
        text: String,
        tint: Color,
        disabled: Bool,
        action: @escaping () -> Void = {}
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: icon)
                    .font(.system(size: 9, weight: .bold))
                Text(text)
                    .font(.system(size: 10, weight: .semibold))
            }
            .foregroundStyle(tint)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
            .overlay(RoundedRectangle(cornerRadius: 5).stroke(tint.opacity(0.35), lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: 5))
        }
        .buttonStyle(.plain)
        .disabled(disabled)
    }
}

struct PRPopover: View {
    @Bindable var state: VCSTabState
    let info: GitRepositoryService.PRInfo
    let onMerge: (GitRepositoryService.PRMergeMethod) -> Void
    let onClose: () -> Void
    let onOpenInBrowser: () -> Void
    let onRefresh: () -> Void

    @State private var mergeMethod: GitRepositoryService.PRMergeMethod = .squash

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: stateIcon)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(stateColor)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Pull Request #\(info.number)")
                        .font(.system(size: 12, weight: .semibold))
                    Text(stateLabel)
                        .font(.system(size: 10))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                Spacer(minLength: 0)
                Button {
                    onRefresh()
                } label: {
                    Group {
                        if state.isRefreshingPullRequest {
                            ProgressView().controlSize(.mini)
                        } else {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(MuxyTheme.fgMuted)
                        }
                    }
                    .frame(width: 20, height: 20)
                }
                .buttonStyle(.plain)
                .disabled(state.isRefreshingPullRequest)
                .help("Refresh")
            }

            VStack(alignment: .leading, spacing: 4) {
                infoRow(label: "Base", value: info.baseBranch)
                if let label = mergeableLabel {
                    infoRow(
                        label: "Mergeable",
                        value: label.text,
                        valueColor: label.color
                    )
                }
                checksRow
            }

            Divider()

            Button(action: onOpenInBrowser) {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.up.right.square")
                        .font(.system(size: 11, weight: .semibold))
                    Text("Open on GitHub")
                        .font(.system(size: 11, weight: .medium))
                    Spacer(minLength: 0)
                }
                .foregroundStyle(MuxyTheme.fg)
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
                .frame(maxWidth: .infinity)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)

            if info.state == .open {
                SegmentedPicker(
                    selection: $mergeMethod,
                    options: GitRepositoryService.PRMergeMethod.allCases.map { ($0, $0.shortLabel) }
                )

                Button { onMerge(mergeMethod) } label: {
                    HStack(spacing: 6) {
                        if state.isMergingPullRequest {
                            ProgressView().controlSize(.mini)
                        } else {
                            Image(systemName: "arrow.triangle.merge")
                                .font(.system(size: 11, weight: .bold))
                        }
                        Text(state.isMergingPullRequest ? "Merging…" : mergeMethod.label)
                            .font(.system(size: 11, weight: .medium))
                        Spacer(minLength: 0)
                    }
                    .foregroundStyle(mergeDisabled ? MuxyTheme.fgDim : MuxyTheme.bg)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity)
                    .background(
                        mergeDisabled ? MuxyTheme.surface : MuxyTheme.accent,
                        in: RoundedRectangle(cornerRadius: 5)
                    )
                }
                .buttonStyle(.plain)
                .disabled(mergeDisabled)
                .help(mergeHelp)

                Button(action: onClose) {
                    HStack(spacing: 6) {
                        if state.isClosingPullRequest {
                            ProgressView().controlSize(.mini)
                        } else {
                            Image(systemName: "xmark.circle")
                                .font(.system(size: 11, weight: .semibold))
                        }
                        Text("Close PR")
                            .font(.system(size: 11, weight: .medium))
                        Spacer(minLength: 0)
                    }
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
                }
                .buttonStyle(.plain)
                .disabled(state.isClosingPullRequest)
            }
        }
        .padding(12)
        .frame(width: 260)
        .task(id: info.number) {
            onRefresh()
        }
    }

    private var mergeDisabled: Bool {
        if state.isMergingPullRequest { return true }
        if info.mergeable == false { return true }
        switch info.mergeStateStatus {
        case .dirty,
             .blocked,
             .behind,
             .draft: return true
        case .clean,
             .hasHooks,
             .unstable,
             .unknown: return false
        }
    }

    private var mergeHelp: String {
        if info.mergeable == false { return "This PR has conflicts and cannot be merged." }
        switch info.mergeStateStatus {
        case .dirty: return "This PR has conflicts and cannot be merged."
        case .behind: return "This branch is out of date with the base branch. Update it before merging."
        case .blocked: return "Merging is blocked by branch protection (required reviews or checks)."
        case .draft: return "This PR is a draft. Mark it ready for review before merging."
        case .unstable:
            return "Checks are failing or pending. You will be asked to confirm before merging."
        case .clean,
             .hasHooks,
             .unknown:
            if info.checks.status == .failure {
                return "Checks are failing. You will be asked to confirm before merging."
            }
            if info.checks.status == .pending {
                return "Checks are still running. You will be asked to confirm before merging."
            }
            return "Merge PR #\(info.number)"
        }
    }

    private var mergeableLabel: (text: String, color: Color)? {
        switch info.mergeStateStatus {
        case .dirty:
            return ("Conflicts", MuxyTheme.diffRemoveFg)
        case .behind:
            return ("Behind base", MuxyTheme.diffRemoveFg)
        case .blocked:
            return ("Blocked", MuxyTheme.diffRemoveFg)
        case .draft:
            return ("Draft", MuxyTheme.fgMuted)
        case .clean,
             .hasHooks:
            return ("Yes", MuxyTheme.diffAddFg)
        case .unstable:
            return ("Yes (checks failing)", MuxyTheme.diffAddFg)
        case .unknown:
            if info.mergeable == true { return ("Yes", MuxyTheme.diffAddFg) }
            if info.mergeable == false { return ("Conflicts", MuxyTheme.diffRemoveFg) }
            return nil
        }
    }

    @ViewBuilder
    private var checksRow: some View {
        switch info.checks.status {
        case .none:
            EmptyView()
        case .success:
            infoRow(
                label: "Checks",
                value: "\(info.checks.passing)/\(info.checks.total) passing",
                valueColor: MuxyTheme.diffAddFg
            )
        case .pending:
            infoRow(
                label: "Checks",
                value: "\(info.checks.pending) running",
                valueColor: MuxyTheme.fgMuted
            )
        case .failure:
            infoRow(
                label: "Checks",
                value: "\(info.checks.failing) failing",
                valueColor: MuxyTheme.diffRemoveFg
            )
        }
    }

    private var stateIcon: String {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return "xmark.octagon.fill"
            case .pending: return "clock"
            default: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? "pencil.circle" : "arrow.triangle.pull"
        case .merged: return "checkmark.circle.fill"
        case .closed: return "xmark.circle"
        }
    }

    private var stateColor: Color {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return MuxyTheme.diffRemoveFg
            case .pending: return MuxyTheme.fgMuted
            default: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? MuxyTheme.fgMuted : MuxyTheme.diffAddFg
        case .merged: return MuxyTheme.accent
        case .closed: return MuxyTheme.diffRemoveFg
        }
    }

    private var stateLabel: String {
        switch info.state {
        case .open: info.isDraft ? "Draft · Open" : "Open"
        case .merged: "Merged"
        case .closed: "Closed"
        }
    }

    private func infoRow(label: String, value: String, valueColor: Color = MuxyTheme.fg) -> some View {
        HStack(spacing: 6) {
            Text(label)
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 70, alignment: .leading)
            Text(value)
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(valueColor)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: 0)
        }
    }
}

private struct SectionSplitLayout: View {
    @Bindable var state: VCSTabState
    let onFocus: () -> Void
    @Binding var showDiscardAllConfirmation: Bool
    @Binding var pendingDiscardPath: String?
    @Binding var pendingCheckoutPR: GitRepositoryService.PRListItem?
    let onOpenInEditor: (String) -> Void
    let onOpenDiff: (String, Bool) -> Void

    private static let sectionHeaderHeight: CGFloat = 30

    private var hasStaged: Bool { !state.stagedFiles.isEmpty }

    private var sections: [SectionKind] {
        var result: [SectionKind] = []
        if hasStaged { result.append(.staged) }
        if state.changesVisible { result.append(.changes) }
        if state.pullRequestsVisible { result.append(.pullRequests) }
        if state.historyVisible { result.append(.history) }
        return result
    }

    private func isCollapsed(_ kind: SectionKind) -> Bool {
        switch kind {
        case .staged: state.stagedCollapsed
        case .changes: state.changesCollapsed
        case .history: state.historyCollapsed
        case .pullRequests: state.pullRequestsCollapsed
        }
    }

    private func toggleCollapsed(_ kind: SectionKind) {
        switch kind {
        case .staged: state.stagedCollapsed.toggle()
        case .changes: state.changesCollapsed.toggle()
        case .history:
            state.historyCollapsed.toggle()
            if !state.historyCollapsed, state.commits.isEmpty {
                state.loadCommits()
            }
        case .pullRequests:
            state.pullRequestsCollapsed.toggle()
        }
    }

    var body: some View {
        GeometryReader { geo in
            let allSections = sections
            let expandedSections = allSections.filter { !isCollapsed($0) }
            let collapsedSections = allSections.filter { isCollapsed($0) }
            let collapsedHeight = CGFloat(collapsedSections.count) * Self.sectionHeaderHeight
            let borderCount = CGFloat(allSections.count + 1)
            let availableForExpanded = max(0, geo.size.height - collapsedHeight - borderCount)
            let ratios = distributedRatios(allSections: allSections, expandedSections: expandedSections)

            VStack(spacing: 0) {
                ForEach(Array(allSections.enumerated()), id: \.element) { index, section in
                    let collapsed = isCollapsed(section)
                    let prevExpanded = previousExpandedSection(before: index, in: allSections)
                    let needsDraggableDivider = !collapsed && prevExpanded != nil

                    if needsDraggableDivider, let prev = prevExpanded {
                        sectionDivider(
                            above: prev,
                            below: section,
                            totalHeight: availableForExpanded,
                            allSections: allSections
                        )
                    } else {
                        Rectangle().fill(MuxyTheme.border).frame(height: 1)
                    }

                    if collapsed {
                        sectionHeader(for: section, collapsed: true)
                            .frame(height: Self.sectionHeaderHeight)
                    } else {
                        let ratio = ratios[section] ?? 0
                        let sectionHeight = max(Self.sectionHeaderHeight, availableForExpanded * ratio)
                        sectionView(for: section, height: sectionHeight)
                    }
                }
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
            }
        }
    }

    private func distributedRatios(
        allSections: [SectionKind],
        expandedSections: [SectionKind]
    ) -> [SectionKind: CGFloat] {
        guard !expandedSections.isEmpty else { return [:] }

        let rawRatios: [CGFloat] = allSections.enumerated().compactMap { idx, section in
            guard !isCollapsed(section) else { return nil }
            return state.sectionRatios[safe: idx] ?? (1.0 / CGFloat(expandedSections.count))
        }

        let sum = rawRatios.reduce(0, +)
        guard sum > 0 else { return [:] }

        var result: [SectionKind: CGFloat] = [:]
        var rawIdx = 0
        for section in expandedSections {
            result[section] = rawRatios[rawIdx] / sum
            rawIdx += 1
        }
        return result
    }

    private func previousExpandedSection(before index: Int, in allSections: [SectionKind]) -> SectionKind? {
        for i in stride(from: index - 1, through: 0, by: -1) where !isCollapsed(allSections[i]) {
            return allSections[i]
        }
        return nil
    }

    private func sectionDivider(
        above: SectionKind,
        below: SectionKind,
        totalHeight: CGFloat,
        allSections: [SectionKind]
    ) -> some View {
        Rectangle().fill(MuxyTheme.border).frame(height: 1)
            .overlay {
                Color.clear
                    .frame(height: 5)
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 1)
                            .onChanged { v in
                                guard totalHeight > 0 else { return }
                                let delta = v.translation.height / totalHeight

                                guard let aboveIdx = allSections.firstIndex(of: above),
                                      let belowIdx = allSections.firstIndex(of: below)
                                else { return }

                                var ratios = state.sectionRatios
                                if ratios.count < allSections.count {
                                    let fill = 1.0 / CGFloat(allSections.count)
                                    ratios.append(contentsOf: Array(repeating: fill, count: allSections.count - ratios.count))
                                }
                                guard aboveIdx < ratios.count, belowIdx < ratios.count else { return }
                                let minRatio: CGFloat = 0.08

                                ratios[aboveIdx] += delta
                                ratios[belowIdx] -= delta

                                ratios[aboveIdx] = max(minRatio, ratios[aboveIdx])
                                ratios[belowIdx] = max(minRatio, ratios[belowIdx])

                                let sum = ratios.reduce(0, +)
                                if sum > 0 {
                                    ratios = ratios.map { $0 / sum }
                                }

                                state.sectionRatios = ratios
                            }
                    )
                    .onHover { on in
                        if on { NSCursor.resizeUpDown.push() } else { NSCursor.pop() }
                    }
            }
    }

    @ViewBuilder
    private func sectionView(for section: SectionKind, height: CGFloat) -> some View {
        switch section {
        case .staged:
            VStack(spacing: 0) {
                sectionHeader(for: .staged, collapsed: false)
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(state.stagedFiles) { file in
                            fileSection(file, isStaged: true)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .frame(height: height)

        case .changes:
            VStack(spacing: 0) {
                sectionHeader(for: .changes, collapsed: false)
                if state.files.isEmpty {
                    Text("No changes")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(state.unstagedFiles) { file in
                                fileSection(file, isStaged: false)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            .frame(height: height)

        case .history:
            VStack(spacing: 0) {
                sectionHeader(for: .history, collapsed: false)
                CommitHistoryView(state: state)
            }
            .frame(height: height)

        case .pullRequests:
            VStack(spacing: 0) {
                sectionHeader(for: .pullRequests, collapsed: false)
                PullRequestsListView(
                    state: state,
                    onCheckout: { pr in pendingCheckoutPR = pr }
                )
            }
            .frame(height: height)
        }
    }

    private func sectionHeader(for section: SectionKind, collapsed: Bool) -> some View {
        let isCollapsedState = collapsed

        return HStack(spacing: 6) {
            Button {
                toggleCollapsed(section)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: isCollapsedState ? "chevron.right" : "chevron.down")
                        .font(.system(size: 9, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .frame(width: 10)

                    Text(section.title)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }
            .buttonStyle(.plain)

            Text("\(sectionCount(for: section))")
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(MuxyTheme.bg)
                .padding(.horizontal, 6)
                .padding(.vertical, 1)
                .background(MuxyTheme.fgMuted, in: Capsule())

            Spacer(minLength: 0)

            sectionActions(for: section)
        }
        .padding(.horizontal, 10)
        .frame(height: Self.sectionHeaderHeight)
        .background(MuxyTheme.bg)
    }

    private func sectionCount(for section: SectionKind) -> Int {
        switch section {
        case .staged: state.stagedFiles.count
        case .changes: state.unstagedFiles.count
        case .history: state.commits.count
        case .pullRequests: state.filteredPullRequests.count
        }
    }

    @ViewBuilder
    private func sectionActions(for section: SectionKind) -> some View {
        switch section {
        case .staged:
            diffModeToggle
            expandCollapseButton(for: state.stagedFiles)
            IconButton(symbol: "minus", size: 11, accessibilityLabel: "Unstage All") {
                state.unstageAll()
            }
            .help("Unstage all")

        case .changes:
            diffModeToggle
            expandCollapseButton(for: state.unstagedFiles)
            IconButton(symbol: "plus", size: 11, accessibilityLabel: "Stage All") {
                state.stageAll()
            }
            .help("Stage all")

            IconButton(symbol: "arrow.uturn.backward", size: 11, accessibilityLabel: "Discard All Changes") {
                showDiscardAllConfirmation = true
            }
            .help("Discard all changes")

        case .history:
            IconButton(symbol: "arrow.clockwise", size: 11, accessibilityLabel: "Refresh History") {
                state.loadCommits()
            }
            .help("Refresh history")

        case .pullRequests:
            PullRequestsAutoSyncMenu(state: state)
            if state.isLoadingPullRequests {
                ProgressView().controlSize(.mini)
            } else {
                IconButton(symbol: "arrow.clockwise", size: 11, accessibilityLabel: "Sync Pull Requests") {
                    state.loadPullRequests()
                }
                .help("Sync pull requests")
            }
        }
    }

    private var diffModeToggle: some View {
        Button {
            state.mode = state.mode == .unified ? .split : .unified
        } label: {
            Image(systemName: state.mode == .unified ? "rectangle.split.2x1" : "rectangle")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 18, height: 18)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(state.mode == .unified ? "Switch to Split View" : "Switch to Unified View")
    }

    @ViewBuilder
    private func expandCollapseButton(for files: [GitStatusFile]) -> some View {
        let anyExpanded = files.contains { state.expandedFilePaths.contains($0.path) }
        Button {
            state.setExpanded(files: files, expanded: !anyExpanded)
        } label: {
            Image(systemName: anyExpanded ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 18, height: 18)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(anyExpanded ? "Collapse all" : "Expand all")
    }

    private func fileSection(_ file: GitStatusFile, isStaged: Bool) -> some View {
        let expanded = state.expandedFilePaths.contains(file.path)
        let stats = state.displayedStats(for: file)
        let statusText = isStaged ? file.stagedStatusText : file.unstagedStatusText

        return VStack(spacing: 0) {
            FileRow(
                file: file,
                statusText: statusText,
                expanded: expanded,
                stats: stats,
                isStaged: isStaged,
                onToggle: {
                    onFocus()
                    state.toggleExpanded(filePath: file.path)
                },
                onStage: { state.stageFile(file.path) },
                onUnstage: { state.unstageFile(file.path) },
                onDiscard: { pendingDiscardPath = file.path },
                onOpenInEditor: { onOpenInEditor(file.path) },
                onOpenDiff: { onOpenDiff(file.path, isStaged) }
            )

            if expanded {
                expandedDiff(for: file)
            }

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func expandedDiff(for file: GitStatusFile) -> some View {
        DiffBodyView(
            isLoading: state.diffCache.isLoading(file.path),
            error: state.diffCache.error(for: file.path),
            diff: state.diffCache.diff(for: file.path),
            filePath: file.path,
            mode: state.mode,
            onLoadFull: { state.loadFullDiff(filePath: file.path) }
        )
    }
}

private enum SectionKind: Hashable {
    case staged
    case changes
    case history
    case pullRequests

    var title: String {
        switch self {
        case .staged: "Staged Changes"
        case .changes: "Changes"
        case .history: "History"
        case .pullRequests: "Pull Requests"
        }
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

private struct FileRow: View {
    let file: GitStatusFile
    let statusText: String
    let expanded: Bool
    let stats: VCSTabState.FileStats
    let isStaged: Bool
    let onToggle: () -> Void
    let onStage: () -> Void
    let onUnstage: () -> Void
    let onDiscard: () -> Void
    let onOpenInEditor: () -> Void
    let onOpenDiff: () -> Void
    @State private var hovered = false

    private var statusColor: Color {
        switch statusText.first {
        case "A":
            MuxyTheme.diffAddFg
        case "D":
            MuxyTheme.diffRemoveFg
        case "M":
            MuxyTheme.accent
        case "R":
            MuxyTheme.accent
        case "U":
            MuxyTheme.diffAddFg
        default:
            MuxyTheme.fgMuted
        }
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: expanded ? "chevron.down" : "chevron.right")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(width: 12)

            Text(statusText)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .foregroundStyle(statusColor)
                .frame(width: 14)

            FileDiffIcon()
                .stroke(statusColor, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                .frame(width: 11, height: 11)

            Text(file.path)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(maxWidth: .infinity, alignment: .leading)

            if hovered {
                actionButtons
            }

            if stats.binary {
                Text("Binary")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgMuted)
            } else {
                if let additions = stats.additions {
                    Text("+\(additions)")
                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.diffAddFg)
                }
                if let deletions = stats.deletions {
                    Text("-\(deletions)")
                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.diffRemoveFg)
                }
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 34)
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .onTapGesture(perform: onToggle)
    }

    private var actionButtons: some View {
        HStack(spacing: 0) {
            IconButton(symbol: "doc.text", size: 11, accessibilityLabel: "Open in Editor", action: onOpenInEditor)
                .help("Open in Editor")
            IconButton(symbol: "rectangle.split.2x1", size: 11, accessibilityLabel: "Open Diff in New Tab", action: onOpenDiff)
                .help("Open Diff in New Tab")
            if isStaged {
                IconButton(symbol: "minus", size: 11, accessibilityLabel: "Unstage", action: onUnstage)
                    .help("Unstage")
            } else {
                IconButton(symbol: "plus", size: 11, accessibilityLabel: "Stage", action: onStage)
                    .help("Stage")
                IconButton(symbol: "arrow.uturn.backward", size: 11, accessibilityLabel: "Discard Changes", action: onDiscard)
                    .help("Discard changes")
            }
        }
    }
}
