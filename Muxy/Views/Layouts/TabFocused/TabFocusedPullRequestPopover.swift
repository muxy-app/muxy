import SwiftUI

struct TabFocusedPullRequestPopover: View {
    let confirmationContext: PullRequestActionConfirmation.Context
    let hasLocalChanges: Bool
    let isRefreshing: Bool
    let isMerging: Bool
    let isClosing: Bool
    let isUpdatingBranch: Bool
    let isWorktreeRemovalInProgress: Bool
    let onMerge: (PullRequestActionConfirmation.Context, GitRepositoryService.PRMergeMethod) -> Void
    let onClose: (PullRequestActionConfirmation.Context) -> Void
    let onOpenInBrowser: () -> Void
    let onRefresh: () -> Void
    let onUpdateBranch: () -> Void

    @State private var mergeMethod: GitRepositoryService.PRMergeMethod = .squash
    @State private var confirmationState = PullRequestActionConfirmation.State()
    @State private var confirmationProgress: CGFloat = 0
    @State private var remainingSeconds = Int(PullRequestActionConfirmation.duration)
    @State private var confirmationTask: Task<Void, Never>?
    @State private var isCancelHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            header
            details
            Divider().overlay(MuxyTheme.border)
            openOnGitHubButton
            if info.state == .open {
                openPullRequestActions
            }
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(280))
        .background(MuxyTheme.bg)
        .task(id: info.number) {
            onRefresh()
        }
        .onChange(of: confirmationContext) { _, _ in
            cancelConfirmation()
        }
        .onChange(of: mergeMethod) { _, _ in
            cancelConfirmation()
        }
        .onChange(of: isPerformingAction) { _, isPerforming in
            if isPerforming {
                cancelConfirmation()
            }
        }
        .onChange(of: isRefreshing) { _, isRefreshing in
            if isRefreshing {
                cancelConfirmation()
            }
        }
        .onDisappear {
            cancelConfirmation()
        }
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: PullRequestPresentation.symbol(for: info))
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(PullRequestPresentation.color(for: info))
            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                Text("Pull Request #\(info.number)")
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Text(PullRequestPresentation.stateLabel(for: info))
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            Spacer(minLength: 0)
            Button(action: onRefresh) {
                Group {
                    if isRefreshing {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: "arrow.clockwise")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    }
                }
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
            }
            .buttonStyle(.plain)
            .disabled(isRefreshing || isPerformingAction || pendingConfirmation != nil)
            .help("Refresh pull request")
        }
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            infoRow(label: "Base", value: info.baseBranch)
            if let mergeability = PRMergeabilityPresentation.make(info: info) {
                infoRow(label: "Merge", value: mergeability.text, valueColor: mergeability.color)
            }
            if let checksLabel = PullRequestPresentation.checksLabel(for: info.checks) {
                infoRow(label: "Checks", value: checksLabel, valueColor: checksColor)
            }
            if hasLocalChanges {
                infoRow(label: "Local", value: "Uncommitted changes", valueColor: MuxyTheme.warning)
            }
        }
    }

    private var openOnGitHubButton: some View {
        actionButton(
            symbol: "arrow.up.right.square",
            label: "Open on GitHub",
            foreground: MuxyTheme.fg,
            background: MuxyTheme.surface,
            action: onOpenInBrowser
        )
    }

    @ViewBuilder
    private var openPullRequestActions: some View {
        if info.mergeStateStatus == .behind, !info.isCrossRepository {
            actionButton(
                symbol: "arrow.down.circle",
                label: isUpdatingBranch ? "Updating…" : "Update from \(info.baseBranch)",
                foreground: MuxyTheme.fg,
                background: MuxyTheme.surface,
                isBusy: isUpdatingBranch,
                action: onUpdateBranch
            )
            .disabled(isRefreshing || isPerformingAction || hasLocalChanges || pendingConfirmation != nil)
            .help(
                hasLocalChanges
                    ? "Commit or stash local changes before updating the branch."
                    : "Merge \(info.baseBranch) into this branch and push it."
            )
        }

        SegmentedPicker(
            selection: $mergeMethod,
            options: GitRepositoryService.PRMergeMethod.allCases.map { ($0, $0.shortLabel) }
        )
        .disabled(isRefreshing || isPerformingAction || pendingConfirmation != nil)

        actionButton(
            symbol: "arrow.triangle.merge",
            label: mergeButtonLabel,
            foreground: mergeDisabled ? MuxyTheme.fgDim : MuxyTheme.accentForeground,
            background: mergeDisabled ? MuxyTheme.surface : MuxyTheme.accent,
            confirmationAction: .merge(mergeMethod),
            confirmationColor: MuxyTheme.accent,
            isBusy: isMerging,
            action: { requestAction(.merge(mergeMethod)) }
        )
        .disabled(mergeDisabled)
        .help(mergeButtonHelp)

        actionButton(
            symbol: "xmark.circle",
            label: closeButtonLabel,
            foreground: MuxyTheme.diffRemoveFg,
            background: MuxyTheme.surface,
            confirmationAction: .close,
            confirmationColor: MuxyTheme.diffRemoveFg,
            isBusy: isClosing,
            action: { requestAction(.close) }
        )
        .disabled(isRefreshing || isPerformingAction)
        .help(closeButtonHelp)
    }

    private func actionButton(
        symbol: String,
        label: String,
        foreground: Color,
        background: Color,
        confirmationAction: PullRequestActionConfirmation.Kind? = nil,
        confirmationColor: Color? = nil,
        isBusy: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        let isConfirming = confirmationAction.map { pendingConfirmation?.kind == $0 } ?? false
        let visibleForeground = isConfirming ? confirmationColor ?? foreground : foreground
        let visibleBackground = isConfirming ? MuxyTheme.surface : background
        return HStack(spacing: isConfirming ? UIMetrics.spacing2 : 0) {
            Button(action: action) {
                HStack(spacing: UIMetrics.spacing3) {
                    if isBusy {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: symbol)
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    }
                    Text(label)
                        .font(.system(size: UIMetrics.fontFootnote, weight: isConfirming ? .semibold : .medium))
                    Spacer(minLength: 0)
                }
                .foregroundStyle(visibleForeground)
                .padding(.horizontal, UIMetrics.spacing4)
                .padding(.vertical, UIMetrics.spacing3)
                .frame(maxWidth: .infinity)
                .background {
                    GeometryReader { geometry in
                        ZStack(alignment: .leading) {
                            RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                                .fill(visibleBackground)
                            if isConfirming, let confirmationColor {
                                Rectangle()
                                    .fill(confirmationColor.opacity(0.22))
                                    .frame(width: geometry.size.width * confirmationProgress)
                            }
                        }
                        .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                    }
                }
                .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            }
            .frame(maxWidth: .infinity)
            .buttonStyle(.plain)
            if isConfirming {
                confirmationCancelButton
            }
        }
    }

    private var confirmationCancelButton: some View {
        Button(action: cancelConfirmation) {
            Image(systemName: "xmark")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.spacing4)
                .padding(.vertical, UIMetrics.spacing3)
                .frame(maxHeight: .infinity)
                .background(
                    isCancelHovered ? MuxyTheme.hover : MuxyTheme.surface,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                )
                .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .buttonStyle(.plain)
        .keyboardShortcut(.cancelAction)
        .onHover { isCancelHovered = $0 }
        .help("Cancel this pending pull request action")
        .accessibilityLabel("Cancel pending pull request action")
    }

    private func requestAction(_ action: PullRequestActionConfirmation.Kind) {
        guard !isRefreshing, !isPerformingAction else { return }
        switch PullRequestActionConfirmation.activation(pending: pendingConfirmation, requested: action) {
        case let .arm(action):
            armConfirmation(for: action)
        case .confirm:
            performAction(action)
        }
    }

    private func armConfirmation(for action: PullRequestActionConfirmation.Kind) {
        cancelConfirmation()
        let confirmation = confirmationState.arm(action)
        let totalSeconds = Int(PullRequestActionConfirmation.duration)
        remainingSeconds = totalSeconds
        confirmationTask = Task { @MainActor in
            await Task.yield()
            guard pendingConfirmation == confirmation else { return }
            withAnimation(.linear(duration: PullRequestActionConfirmation.duration)) {
                confirmationProgress = 1
            }
            for second in stride(from: totalSeconds - 1, through: 1, by: -1) {
                do {
                    try await Task.sleep(nanoseconds: 1_000_000_000)
                } catch {
                    return
                }
                guard pendingConfirmation == confirmation else { return }
                remainingSeconds = second
            }
            do {
                try await Task.sleep(nanoseconds: 1_000_000_000)
            } catch {
                return
            }
            guard pendingConfirmation == confirmation else { return }
            performAction(action)
        }
    }

    private func performAction(_ action: PullRequestActionConfirmation.Kind) {
        cancelConfirmation()
        switch action {
        case let .merge(method):
            onMerge(confirmationContext, method)
        case .close:
            onClose(confirmationContext)
        }
    }

    private func cancelConfirmation() {
        confirmationTask?.cancel()
        confirmationTask = nil
        var transaction = Transaction()
        transaction.disablesAnimations = true
        withTransaction(transaction) {
            confirmationState.cancel()
            confirmationProgress = 0
            remainingSeconds = Int(PullRequestActionConfirmation.duration)
            isCancelHovered = false
        }
    }

    private var mergeAvailability: PRMergeAvailability {
        PRMergeAvailability.make(info: info)
    }

    private var mergeDisabled: Bool {
        isRefreshing || isPerformingAction || !mergeAvailability.isEnabled
    }

    private var mergeButtonLabel: String {
        if isMerging {
            return "Merging…"
        }
        if pendingConfirmation?.kind == .merge(mergeMethod) {
            return "\(mergeMethod.shortLabel) in \(remainingSeconds)s · click again"
        }
        return mergeMethod.label
    }

    private var mergeButtonHelp: String {
        if isRefreshing {
            return "Wait for the pull request refresh to finish."
        }
        guard pendingConfirmation?.kind == .merge(mergeMethod) else { return mergeAvailability.help }
        return "Click again to merge now. Otherwise it will merge automatically after five seconds."
    }

    private var closeButtonLabel: String {
        if isClosing {
            return "Closing…"
        }
        if pendingConfirmation?.kind == .close {
            return "Close in \(remainingSeconds)s · click again"
        }
        return "Close PR"
    }

    private var closeButtonHelp: String {
        if isRefreshing {
            return "Wait for the pull request refresh to finish."
        }
        guard pendingConfirmation?.kind == .close else { return "Close this pull request without merging it." }
        return "Click again to close now. Otherwise it will close automatically after five seconds."
    }

    private var isPerformingAction: Bool {
        isMerging || isClosing || isUpdatingBranch || isWorktreeRemovalInProgress
    }

    private var info: GitRepositoryService.PRInfo {
        confirmationContext.pullRequest
    }

    private var pendingConfirmation: PullRequestActionConfirmation.Pending? {
        confirmationState.pending
    }

    private var checksColor: Color {
        switch info.checks.status {
        case .none: MuxyTheme.fgMuted
        case .success: MuxyTheme.diffAddFg
        case .pending: MuxyTheme.warning
        case .failure: MuxyTheme.diffRemoveFg
        }
    }

    private func infoRow(label: String, value: String, valueColor: Color = MuxyTheme.fg) -> some View {
        HStack(spacing: UIMetrics.spacing3) {
            Text(label)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(52), alignment: .leading)
            Text(value)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .monospaced))
                .foregroundStyle(valueColor)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: 0)
        }
    }
}
