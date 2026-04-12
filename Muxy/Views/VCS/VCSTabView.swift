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
    @State private var showCreatePRSheet = false
    @State private var showWorktreePopover = false
    @State private var isGitRepo = false
    @State private var pendingClosePR: PRInfo?
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
            worktreeTrigger

            BranchPicker(
                currentBranch: state.branchName,
                branches: state.branches,
                isLoading: state.isLoadingBranches,
                onSelect: { state.switchBranch($0) },
                onRefresh: { state.loadBranches() },
                onCreateBranch: { showCreateBranchSheet = true },
                onDeleteBranch: { branch in presentDeleteBranchConfirmation(branch) }
            )

            PRPill(
                state: state,
                onRequestCreate: { requestOpenPR() },
                onRequestMerge: { prInfo, method in performMerge(prInfo: prInfo, method: method) },
                onRequestClose: { prInfo in pendingClosePR = prInfo }
            )

            Spacer(minLength: 0)

            IconButton(symbol: "arrow.clockwise") {
                state.refresh()
            }
        }
        .padding(.horizontal, 8)
        .frame(height: 32)
        .background(MuxyTheme.bg)
        .task(id: owningProject?.path) {
            if let path = owningProject?.path {
                isGitRepo = await GitWorktreeService.shared.isGitRepository(path)
            }
        }
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
        .sheet(isPresented: $showCreatePRSheet) {
            CreatePRSheet(
                context: CreatePRSheet.Context(
                    currentBranch: state.branchName ?? "",
                    defaultBranch: state.defaultBranch,
                    availableBaseBranches: state.remoteBranches,
                    isLoadingBranches: state.isLoadingRemoteBranches,
                    hasStagedChanges: state.hasStagedChanges,
                    hasUnstagedChanges: !state.unstagedFiles.isEmpty
                ),
                inProgress: state.isOpeningPullRequest,
                errorMessage: state.openPullRequestError,
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
                    showCreatePRSheet = false
                }
            )
        }
        .onChange(of: state.pullRequestInfo?.number) { _, number in
            guard number != nil, showCreatePRSheet else { return }
            showCreatePRSheet = false
        }
    }

    private func requestOpenPR() {
        state.openPullRequestError = nil
        state.loadRemoteBranches()
        showCreatePRSheet = true
    }

    @ViewBuilder
    private var worktreeTrigger: some View {
        if let project = owningProject {
            Button {
                showWorktreePopover = true
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "square.stack.3d.up")
                        .font(.system(size: 9, weight: .semibold))
                    Text(worktreeTriggerLabel)
                        .font(.system(size: 10, weight: .medium))
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .frame(maxWidth: 120, alignment: .leading)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 8, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                .foregroundStyle(MuxyTheme.fg.opacity(0.85))
                .padding(.horizontal, 6)
                .padding(.vertical, 3)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
                .contentShape(RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)
            .help(worktreeTriggerLabel)
            .popover(isPresented: $showWorktreePopover, arrowEdge: .top) {
                WorktreePopover(
                    project: project,
                    isGitRepo: isGitRepo,
                    onDismiss: { showWorktreePopover = false },
                    onRequestCreate: {
                        showWorktreePopover = false
                        showCreateWorktreeSheet = true
                    }
                )
                .environment(appState)
                .environment(worktreeStore)
            }
        }
    }

    private var worktreeTriggerLabel: String {
        guard let worktree = activeWorktreeForTab else { return "default" }
        if worktree.isPrimary {
            return worktree.name.isEmpty ? "default" : worktree.name
        }
        return worktree.name
    }

    private func performMerge(prInfo: PRInfo, method: PRMergeMethod) {
        if prInfo.checks.status == .failure || prInfo.checks.status == .pending {
            presentChecksMergeConfirmation(prInfo: prInfo, method: method)
            return
        }
        continueMergeAfterChecks(prInfo: prInfo, method: method)
    }

    private func continueMergeAfterChecks(prInfo: PRInfo, method: PRMergeMethod) {
        if state.hasAnyChanges {
            presentDirtyMergeConfirmation(prInfo: prInfo, method: method)
            return
        }
        executeMerge(prInfo: prInfo, method: method)
    }

    private func presentChecksMergeConfirmation(prInfo: PRInfo, method: PRMergeMethod) {
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

    private func executeMerge(prInfo: PRInfo, method: PRMergeMethod) {
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

    private func presentDirtyMergeConfirmation(prInfo: PRInfo, method: PRMergeMethod) {
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
        if let project, let worktree, !worktree.isPrimary {
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
            try? await GitPullRequestService().deleteRemoteBranch(
                repoPath: repoPath,
                branch: mergedBranch
            )
        }
    }

    private func presentClosePRConfirmation(prInfo: PRInfo) {
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
                commitArea
                VCSSectionSplitLayout(
                    state: state,
                    onFocus: onFocus,
                    showDiscardAllConfirmation: $showDiscardAllConfirmation,
                    pendingDiscardPath: $pendingDiscardPath,
                    onOpenInEditor: openFileInEditor
                )
            }
        }
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
                    .frame(minHeight: 54, maxHeight: 100)
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

    private func openFileInEditor(_ relativePath: String) {
        guard let projectID = appState.activeProjectID else { return }
        let fullPath = state.projectPath.hasSuffix("/")
            ? state.projectPath + relativePath
            : state.projectPath + "/" + relativePath
        appState.openFile(fullPath, projectID: projectID)
    }
}
