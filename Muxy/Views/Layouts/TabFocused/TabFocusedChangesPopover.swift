import SwiftUI

struct TabFocusedChangesPopover: View {
    let summary: GitRepositorySummary
    let files: [GitStatusFile]
    let error: String?
    let isLoading: Bool
    let isMutating: Bool
    let isRepositoryInteractionDisabled: Bool
    let worktreeRemovalState: RepositoryToolbarPresentation.WorktreeRemovalState
    let worktreeRemovalHelp: String?
    let onRefresh: () -> Void
    let onStage: (GitStatusFile) -> Void
    let onStageAll: () -> Void
    let onUnstage: (GitStatusFile) -> Void
    let onUnstageAll: () -> Void
    let onDiscard: (GitStatusFile) -> Void
    let onRemoveWorktree: () -> Void

    @State private var pendingDiscard: GitStatusFile?
    @State private var isRemoveWorktreeHovered = false

    private var stagedFiles: [GitStatusFile] {
        RepositoryChangesPresentation.stagedFiles(files)
    }

    private var unstagedFiles: [GitStatusFile] {
        RepositoryChangesPresentation.unstagedFiles(files)
    }

    private var conflictedFiles: [GitStatusFile] {
        RepositoryChangesPresentation.conflictedFiles(files)
    }

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
        .frame(width: UIMetrics.scaled(440), height: UIMetrics.scaled(500))
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
            }
            Spacer(minLength: UIMetrics.spacing3)
            lineStats(RepositoryChangesPresentation.lineStats(files))
            Button(action: onRefresh) {
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
        .padding(UIMetrics.spacing5)
    }

    @ViewBuilder
    private var content: some View {
        if isLoading, files.isEmpty {
            ProgressView()
                .controlSize(.small)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let error, files.isEmpty {
            errorState(error)
        } else if files.isEmpty {
            cleanState
        } else {
            ScrollView {
                LazyVStack(spacing: UIMetrics.spacing4) {
                    if !conflictedFiles.isEmpty {
                        section(
                            title: "Conflicts",
                            files: conflictedFiles,
                            side: .conflicted,
                            batchAction: nil
                        )
                    }
                    if !stagedFiles.isEmpty {
                        section(
                            title: "Staged",
                            files: stagedFiles,
                            side: .staged,
                            batchAction: ("Unstage All", onUnstageAll)
                        )
                    }
                    if !unstagedFiles.isEmpty {
                        section(
                            title: "Changes",
                            files: unstagedFiles,
                            side: .unstaged,
                            batchAction: ("Stage All", onStageAll)
                        )
                    }
                }
                .padding(.vertical, UIMetrics.spacing4)
            }
        }
    }

    private func section(
        title: String,
        files: [GitStatusFile],
        side: ChangeSide,
        batchAction: (title: String, action: () -> Void)?
    ) -> some View {
        VStack(spacing: UIMetrics.spacing2) {
            HStack(spacing: UIMetrics.spacing3) {
                Text(title)
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(side == .conflicted ? MuxyTheme.warning : MuxyTheme.fgMuted)
                Text("\(files.count)")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .rounded))
                    .foregroundStyle(MuxyTheme.fgDim)
                lineStats(RepositoryChangesPresentation.lineStats(files, staged: side.stagedValue))
                Spacer(minLength: UIMetrics.spacing3)
                if let batchAction {
                    Button(batchAction.title, action: batchAction.action)
                        .buttonStyle(.plain)
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.accent)
                        .disabled(isInteractionDisabled)
                }
            }
            .padding(.horizontal, UIMetrics.spacing5)

            VStack(spacing: UIMetrics.spacing1) {
                ForEach(files) { file in
                    fileRow(file, side: side)
                }
            }
        }
    }

    private func fileRow(_ file: GitStatusFile, side: ChangeSide) -> some View {
        HStack(spacing: UIMetrics.spacing3) {
            Text(file.displayStatusText(isStaged: side == .staged))
                .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .monospaced))
                .foregroundStyle(statusColor(file, side: side))
                .frame(width: UIMetrics.scaled(20), height: UIMetrics.scaled(20))
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))

            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                Text((file.path as NSString).lastPathComponent)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
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
            rowActions(file, side: side)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(44))
        .contentShape(Rectangle())
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
            .font(.system(size: UIMetrics.fontXS, weight: .semibold, design: .monospaced))
            .fixedSize()
            .accessibilityLabel("\(stats.additions) additions, \(stats.deletions) deletions")
        }
    }

    @ViewBuilder
    private func fileLineStats(_ file: GitStatusFile, side: ChangeSide) -> some View {
        if file.isBinary {
            Text("Binary")
                .font(.system(size: UIMetrics.fontXS, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        } else {
            let stats = RepositoryChangesPresentation.lineStats([file], staged: side.stagedValue)
            if stats.hasKnownValues {
                lineStats(stats)
            } else {
                Text("—")
                    .font(.system(size: UIMetrics.fontXS, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .accessibilityLabel("Line counts unavailable")
            }
        }
    }

    private var cleanState: some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "checkmark.circle")
                .font(.system(size: UIMetrics.scaled(28), weight: .medium))
                .foregroundStyle(MuxyTheme.diffAddFg)
            Text("Working tree is clean")
                .font(.system(size: UIMetrics.fontBody, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorState(_ error: String) -> some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: UIMetrics.scaled(24), weight: .medium))
                .foregroundStyle(MuxyTheme.warning)
            Text("Changes unavailable")
                .font(.system(size: UIMetrics.fontBody, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
            Text(error)
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .lineLimit(3)
            Button("Retry", action: onRefresh)
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
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                        .frame(width: UIMetrics.scaled(14))
                    Text(worktreeRemovalLabel)
                        .font(.system(size: UIMetrics.fontBody, weight: .medium))
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

    private func statusColor(_ file: GitStatusFile, side: ChangeSide) -> Color {
        if side == .conflicted { return MuxyTheme.warning }
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
                .font(.system(size: UIMetrics.fontXS, weight: .bold))
                .foregroundStyle(foreground)
                .frame(width: UIMetrics.scaled(24), height: UIMetrics.scaled(24))
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
        if isDisabled { return MuxyTheme.fgDim }
        return isDestructive ? MuxyTheme.diffRemoveFg : MuxyTheme.fgMuted
    }
}
