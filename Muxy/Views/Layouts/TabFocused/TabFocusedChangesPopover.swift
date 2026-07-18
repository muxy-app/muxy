import SwiftUI

struct TabFocusedChangesPopover: View {
    let summary: GitRepositorySummary
    let changes: RepositoryChangesSnapshot
    let untrackedLineStats: [String: Int]
    let untrackedLineStatsSummary: RepositoryChangesLineStats
    let hasLoadedChanges: Bool
    let error: String?
    let isLoading: Bool
    let isMutating: Bool
    let isRepositoryInteractionDisabled: Bool
    let worktreeRemovalState: RepositoryToolbarPresentation.WorktreeRemovalState
    let worktreeRemovalHelp: String?
    let onRefresh: () async -> Void
    let onStage: (GitStatusFile) -> Void
    let onStageAll: () -> Void
    let onUnstage: (GitStatusFile) -> Void
    let onUnstageAll: () -> Void
    let onDiscard: (GitStatusFile) -> Void
    let onLoadLineStats: (GitStatusFile) async -> Void
    let onRemoveWorktree: () -> Void

    @State private var pendingDiscard: GitStatusFile?
    @State private var isRemoveWorktreeHovered = false
    @State private var refreshGeneration = 0

    private var isInteractionDisabled: Bool {
        isLoading || isMutating || isRepositoryInteractionDisabled
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(MuxyTheme.border)
            content
            worktreeRemovalFooter
        }
        .frame(width: UIMetrics.scaled(360), height: UIMetrics.scaled(380))
        .background(MuxyTheme.bg)
        .alert(item: $pendingDiscard) { file in
            Alert(
                title: Text(file.isUntracked ? "Delete \(file.path)?" : "Discard changes to \(file.path)?"),
                message: Text(discardMessage(file)),
                primaryButton: .destructive(Text(file.isUntracked ? "Delete File" : "Discard")) {
                    onDiscard(file)
                },
                secondaryButton: .cancel()
            )
        }
        .task(id: refreshGeneration) {
            await Task.yield()
            guard !Task.isCancelled else { return }
            await onRefresh()
        }
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "arrow.left.arrow.right")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(summary.isDirty ? MuxyTheme.warning : MuxyTheme.diffAddFg)
            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                Text("Changes")
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Text(workingTreeDescription)
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .lineLimit(1)
            }
            .layoutPriority(1)
            Spacer(minLength: UIMetrics.spacing3)
            lineStats(changes.totalLineStats.merging(untrackedLineStatsSummary))
            Button(action: requestRefresh) {
                Group {
                    if isLoading {
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
            .disabled(isInteractionDisabled)
            .help("Refresh working tree changes")
            .accessibilityLabel("Refresh working tree changes")
        }
        .padding(UIMetrics.spacing4)
    }

    @ViewBuilder
    private var content: some View {
        if let error, changes.isEmpty {
            errorState(error)
        } else if changes.isEmpty, isLoading || (summary.isDirty && !hasLoadedChanges) {
            ProgressView()
                .controlSize(.small)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if changes.isEmpty {
            cleanState
        } else {
            ScrollView {
                LazyVStack(spacing: UIMetrics.spacing1) {
                    if !changes.conflictedFiles.isEmpty {
                        section(
                            title: "Conflicts",
                            files: changes.conflictedFiles,
                            lineStats: changes.conflictedLineStats,
                            side: .conflicted,
                            batchAction: nil
                        )
                    }
                    if !changes.stagedFiles.isEmpty {
                        section(
                            title: "Staged",
                            files: changes.stagedFiles,
                            lineStats: changes.stagedLineStats,
                            side: .staged,
                            batchAction: ("Unstage All", onUnstageAll)
                        )
                    }
                    if !changes.unstagedFiles.isEmpty {
                        section(
                            title: "Changes",
                            files: changes.unstagedFiles,
                            lineStats: changes.unstagedLineStats.merging(untrackedLineStatsSummary),
                            side: .unstaged,
                            batchAction: ("Stage All", onStageAll)
                        )
                    }
                }
                .padding(.bottom, UIMetrics.spacing4)
            }
        }
    }

    private func section(
        title: String,
        files: [GitStatusFile],
        lineStats sectionLineStats: RepositoryChangesLineStats,
        side: ChangeSide,
        batchAction: (title: String, action: () -> Void)?
    ) -> some View {
        Section {
            ForEach(files) { file in
                fileRow(file, side: side)
            }
        } header: {
            HStack(spacing: UIMetrics.spacing3) {
                Text(title)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(side == .conflicted ? MuxyTheme.warning : MuxyTheme.fgMuted)
                Text("\(files.count)")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .rounded))
                    .foregroundStyle(MuxyTheme.fgDim)
                lineStats(sectionLineStats)
                Spacer(minLength: UIMetrics.spacing3)
                if let batchAction {
                    Button(batchAction.title, action: batchAction.action)
                        .buttonStyle(.plain)
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.accent)
                        .disabled(isInteractionDisabled)
                }
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.top, UIMetrics.spacing3)
            .padding(.bottom, UIMetrics.spacing2)
            .background(MuxyTheme.bg)
        }
    }

    private func fileRow(_ file: GitStatusFile, side: ChangeSide) -> some View {
        ChangesPopoverFileRow {
            HStack(spacing: UIMetrics.spacing3) {
                Text(file.displayStatusText(isStaged: side == .staged))
                    .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .monospaced))
                    .foregroundStyle(statusColor(file, side: side))
                    .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))

                VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                    Text((file.path as NSString).lastPathComponent)
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                    Text(fileDetail(file))
                        .font(.system(size: UIMetrics.fontXS, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                Spacer(minLength: UIMetrics.spacing2)
                fileLineStats(file, side: side)
                    .frame(minWidth: UIMetrics.scaled(56), alignment: .trailing)
                rowActions(file, side: side)
            }
        }
        .task {
            guard side == .unstaged,
                  file.isUntracked,
                  file.additions == nil
            else { return }
            await onLoadLineStats(file)
        }
    }

    @ViewBuilder
    private func rowActions(_ file: GitStatusFile, side: ChangeSide) -> some View {
        switch side {
        case .conflicted:
            ChangesPopoverActionButton(
                symbol: "plus",
                help: "Stage resolved file \(file.path)",
                isDisabled: isInteractionDisabled,
                action: { onStage(file) }
            )
        case .staged:
            ChangesPopoverActionButton(
                symbol: "minus",
                help: "Unstage \(file.path)",
                isDisabled: isInteractionDisabled,
                action: { onUnstage(file) }
            )
        case .unstaged:
            ChangesPopoverActionButton(
                symbol: "plus",
                help: "Stage \(file.path)",
                isDisabled: isInteractionDisabled,
                action: { onStage(file) }
            )
            ChangesPopoverActionButton(
                symbol: "trash",
                help: file.isUntracked ? "Delete untracked file \(file.path)" : "Discard changes to \(file.path)",
                isDestructive: true,
                isDisabled: isInteractionDisabled,
                action: { pendingDiscard = file }
            )
        }
    }

    @ViewBuilder
    private func lineStats(_ stats: RepositoryChangesLineStats) -> some View {
        if stats.hasKnownValues {
            HStack(spacing: UIMetrics.spacing2) {
                Text("+\(stats.additions)")
                    .foregroundStyle(MuxyTheme.diffAddFg)
                Text("−\(stats.deletions)")
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            }
            .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
            .fixedSize()
            .accessibilityLabel("\(stats.additions) additions, \(stats.deletions) deletions")
        }
    }

    @ViewBuilder
    private func fileLineStats(_ file: GitStatusFile, side: ChangeSide) -> some View {
        if let untrackedLineCount = untrackedLineStats[file.path] {
            lineStats(RepositoryChangesLineStats(
                additions: untrackedLineCount,
                deletions: 0,
                hasKnownValues: true
            ))
        } else if file.isBinary {
            Text("Binary")
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        } else {
            let stats = RepositoryChangesPresentation.lineStats(file, staged: side.stagedValue)
            if stats.hasKnownValues {
                lineStats(stats)
            } else {
                Text("—")
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .accessibilityLabel("Line counts unavailable")
            }
        }
    }

    private var cleanState: some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "checkmark.circle")
                .font(.system(size: UIMetrics.fontDisplay, weight: .medium))
                .foregroundStyle(MuxyTheme.diffAddFg)
            Text("Working tree is clean")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorState(_ error: String) -> some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: UIMetrics.fontDisplay, weight: .medium))
                .foregroundStyle(MuxyTheme.warning)
            Text("Changes unavailable")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
            Text(error)
                .font(.system(size: UIMetrics.fontXS))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .lineLimit(3)
            Button("Retry", action: requestRefresh)
                .buttonStyle(.plain)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(isInteractionDisabled ? MuxyTheme.fgDim : MuxyTheme.accent)
                .disabled(isInteractionDisabled)
        }
        .padding(UIMetrics.spacing6)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private var worktreeRemovalFooter: some View {
        if worktreeRemovalState != .hidden {
            Divider().overlay(MuxyTheme.border)
            Button(role: .destructive, action: onRemoveWorktree) {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "trash")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .frame(width: UIMetrics.iconSM)
                    Text(worktreeRemovalLabel)
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    Spacer(minLength: UIMetrics.spacing3)
                }
                .foregroundStyle(isWorktreeRemovalDisabled ? MuxyTheme.fgMuted : MuxyTheme.diffRemoveFg)
                .padding(.horizontal, UIMetrics.spacing3)
                .frame(height: UIMetrics.controlMedium)
                .background(
                    isRemoveWorktreeHovered && !isWorktreeRemovalDisabled ? MuxyTheme.hover : .clear,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(isWorktreeRemovalDisabled)
            .onHover { isRemoveWorktreeHovered = $0 }
            .help(worktreeRemovalHelp ?? worktreeRemovalLabel)
            .accessibilityLabel(worktreeRemovalHelp ?? worktreeRemovalLabel)
            .padding(UIMetrics.spacing3)
        }
    }

    private var isWorktreeRemovalDisabled: Bool {
        worktreeRemovalState != .available || isInteractionDisabled
    }

    private var worktreeRemovalLabel: String {
        switch worktreeRemovalState {
        case .hidden,
             .available:
            "Remove worktree"
        case .preparing:
            "Checking…"
        case .removing:
            "Removing…"
        }
    }

    private var workingTreeDescription: String {
        guard summary.isDirty else { return "Working tree clean" }
        return "\(summary.changedCount) changed · \(summary.stagedCount) staged · \(summary.untrackedCount) untracked"
    }

    private func fileDetail(_ file: GitStatusFile) -> String {
        if let oldPath = file.oldPath {
            return "\(oldPath) → \(file.path)"
        }
        let directory = (file.path as NSString).deletingLastPathComponent
        return directory.isEmpty ? file.path : directory
    }

    private func discardMessage(_ file: GitStatusFile) -> String {
        if file.isUntracked {
            return "This untracked file will be permanently deleted."
        }
        return "Unstaged changes to this file will be permanently discarded."
    }

    private func requestRefresh() {
        refreshGeneration &+= 1
    }

    private func statusColor(_ file: GitStatusFile, side: ChangeSide) -> Color {
        if side == .conflicted {
            return MuxyTheme.warning
        }
        return switch file.displayStatusText(isStaged: side == .staged) {
        case "A",
             "C": MuxyTheme.diffAddFg
        case "D": MuxyTheme.diffRemoveFg
        case "R": MuxyTheme.accent
        default: MuxyTheme.warning
        }
    }

    private enum ChangeSide: Equatable {
        case conflicted
        case staged
        case unstaged

        var stagedValue: Bool? {
            switch self {
            case .conflicted: nil
            case .staged: true
            case .unstaged: false
            }
        }
    }
}

private struct ChangesPopoverActionButton: View {
    let symbol: String
    let help: String
    var isDestructive = false
    let isDisabled: Bool
    let action: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                .foregroundStyle(foreground)
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                .background(isHovered && !isDisabled ? MuxyTheme.hover : .clear, in: RoundedRectangle(
                    cornerRadius: UIMetrics.radiusSM
                ))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(isDisabled)
        .onHover { isHovered = $0 }
        .help(help)
        .accessibilityLabel(help)
    }

    private var foreground: Color {
        if isDisabled {
            return MuxyTheme.fgDim
        }
        return isDestructive ? MuxyTheme.diffRemoveFg : MuxyTheme.fgMuted
    }
}

private struct ChangesPopoverFileRow<Content: View>: View {
    let content: Content

    @State private var isHovered = false

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.scaled(34))
            .background(
                isHovered ? MuxyTheme.hover : .clear,
                in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
            )
            .padding(.horizontal, UIMetrics.spacing2)
            .contentShape(Rectangle())
            .onHover { isHovered = $0 }
    }
}
