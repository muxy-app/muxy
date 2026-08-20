import AppKit
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
    let onStage: ([GitStatusFile]) -> Void
    let onUnstage: ([GitStatusFile]) -> Void
    let onDiscard: (GitStatusFile) -> Void
    let onLoadLineStats: (GitStatusFile) async -> Void
    let onRemoveWorktree: () -> Void

    @State private var pendingDiscard: GitStatusFile?
    @State private var isRemoveWorktreeHovered = false
    @State private var refreshGeneration = 0
    @State private var conflictedSelection = RepositoryChangesFileSelection()
    @State private var stagedSelection = RepositoryChangesFileSelection()
    @State private var unstagedSelection = RepositoryChangesFileSelection()

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
                title: Text(
                    file.isUntracked
                        ? L10n.resource("Delete \(file.path)?")
                        : L10n.resource("Discard changes to \(file.path)?")
                ),
                message: Text(discardMessage(file)),
                primaryButton: .destructive(
                    Text(
                        file.isUntracked
                            ? L10n.resource("Delete File")
                            : L10n.resource("Discard")
                    )
                ) {
                    onDiscard(file)
                },
                secondaryButton: .cancel()
            )
        }
        .onChange(of: changes.conflictedFiles.map(\.id)) { _, ids in
            conflictedSelection.retain(ids: ids)
        }
        .onChange(of: changes.stagedFiles.map(\.id)) { _, ids in
            stagedSelection.retain(ids: ids)
        }
        .onChange(of: changes.unstagedFiles.map(\.id)) { _, ids in
            unstagedSelection.retain(ids: ids)
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
                Text(L10n.resource("Changes"))
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Text(L10n.resource(workingTreeDescription))
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
            .help(L10n.string("Refresh working tree changes"))
            .accessibilityLabel(L10n.string("Refresh working tree changes"))
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
                            batchActions: conflictedBatchActions
                        )
                    }
                    if !changes.stagedFiles.isEmpty {
                        section(
                            title: "Staged",
                            files: changes.stagedFiles,
                            lineStats: changes.stagedLineStats,
                            side: .staged,
                            batchActions: stagedBatchActions
                        )
                    }
                    if !changes.unstagedFiles.isEmpty {
                        section(
                            title: "Changes",
                            files: changes.unstagedFiles,
                            lineStats: changes.unstagedLineStats.merging(untrackedLineStatsSummary),
                            side: .unstaged,
                            batchActions: unstagedBatchActions
                        )
                    }
                }
                .padding(.bottom, UIMetrics.spacing4)
            }
        }
    }

    private func section(
        title: LocalizedStringResource,
        files: [GitStatusFile],
        lineStats sectionLineStats: RepositoryChangesLineStats,
        side: ChangeSide,
        batchActions: [ChangesPopoverBatchAction]
    ) -> some View {
        Section {
            ForEach(files) { file in
                fileRow(file, side: side)
                    .id(rowID(file, side: side))
            }
        } header: {
            HStack(spacing: UIMetrics.spacing3) {
                Text(L10n.resource(title))
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(side == .conflicted ? MuxyTheme.warning : MuxyTheme.fgMuted)
                Text(L10n.resource("\(files.count)"))
                    .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .rounded))
                    .foregroundStyle(MuxyTheme.fgDim)
                lineStats(sectionLineStats)
                Spacer(minLength: UIMetrics.spacing3)
                HStack(spacing: UIMetrics.spacing4) {
                    ForEach(batchActions.indices, id: \.self) { index in
                        let action = batchActions[index]
                        Button(L10n.string(action.title), action: action.action)
                            .buttonStyle(.plain)
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                            .foregroundStyle(isInteractionDisabled ? MuxyTheme.fgDim : MuxyTheme.accent)
                            .disabled(isInteractionDisabled)
                            .fixedSize()
                    }
                }
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.top, UIMetrics.spacing3)
            .padding(.bottom, UIMetrics.spacing2)
            .background(MuxyTheme.bg)
        }
    }

    private func fileRow(_ file: GitStatusFile, side: ChangeSide) -> some View {
        let isSelected = selection(for: side).contains(file.id)
        return ChangesPopoverFileRow(isSelected: isSelected) {
            HStack(spacing: UIMetrics.spacing3) {
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
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                .contentShape(Rectangle())
                .overlay {
                    LeftClickView { event in
                        handleFileClick(file, side: side, event: event)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .accessibilityHidden(true)
                }
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(isSelected ? [.isButton, .isSelected] : .isButton)
                .accessibilityAction {
                    handleFileClick(file, side: side, kind: .exclusive)
                }

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
                help: L10n.string("Stage resolved file \(file.path)"),
                isDisabled: isInteractionDisabled,
                action: { stage(actionTargets(file, side: side), from: side) }
            )
        case .staged:
            ChangesPopoverActionButton(
                symbol: "minus",
                help: L10n.string("Unstage \(file.path)"),
                isDisabled: isInteractionDisabled,
                action: { unstage(actionTargets(file, side: side), from: side) }
            )
        case .unstaged:
            ChangesPopoverActionButton(
                symbol: "plus",
                help: L10n.string("Stage \(file.path)"),
                isDisabled: isInteractionDisabled,
                action: { stage(actionTargets(file, side: side), from: side) }
            )
            ChangesPopoverActionButton(
                symbol: "trash",
                help: file.isUntracked
                    ? L10n.string("Delete untracked file \(file.path)")
                    : L10n.string("Discard changes to \(file.path)"),
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
                Text(L10n.resource("+\(stats.additions)"))
                    .foregroundStyle(MuxyTheme.diffAddFg)
                Text(L10n.resource("−\(stats.deletions)"))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            }
            .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
            .fixedSize()
            .accessibilityLabel(L10n.string("\(stats.additions) additions, \(stats.deletions) deletions"))
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
            Text(L10n.resource("Binary"))
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        } else {
            let stats = RepositoryChangesPresentation.lineStats(file, staged: side.stagedValue)
            if stats.hasKnownValues {
                lineStats(stats)
            } else {
                Text(L10n.resource("—"))
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .accessibilityLabel(L10n.string("Line counts unavailable"))
            }
        }
    }

    private var cleanState: some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "checkmark.circle")
                .font(.system(size: UIMetrics.fontDisplay, weight: .medium))
                .foregroundStyle(MuxyTheme.diffAddFg)
            Text(L10n.resource("Working tree is clean"))
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
            Text(L10n.resource("Changes unavailable"))
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
            Text(error)
                .font(.system(size: UIMetrics.fontXS))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .lineLimit(3)
            Button(L10n.string("Retry"), action: requestRefresh)
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
                    Text(L10n.resource(worktreeRemovalLabel))
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
            .help(worktreeRemovalHelp ?? L10n.string(worktreeRemovalLabel))
            .accessibilityLabel(worktreeRemovalHelp ?? L10n.string(worktreeRemovalLabel))
            .padding(UIMetrics.spacing3)
        }
    }

    private var isWorktreeRemovalDisabled: Bool {
        worktreeRemovalState != .available || isInteractionDisabled
    }

    private var worktreeRemovalLabel: LocalizedStringResource {
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

    private var workingTreeDescription: LocalizedStringResource {
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
            return L10n.string("This untracked file will be permanently deleted.")
        }
        return L10n.string("Unstaged changes to this file will be permanently discarded.")
    }

    private func requestRefresh() {
        refreshGeneration &+= 1
    }

    private var conflictedBatchActions: [ChangesPopoverBatchAction] {
        guard !conflictedSelection.isEmpty else { return [] }
        return [
            ChangesPopoverBatchAction(
                title: "Stage Selected",
                action: { stage(conflictedSelection.files(in: changes.conflictedFiles), from: .conflicted) }
            ),
        ]
    }

    private var stagedBatchActions: [ChangesPopoverBatchAction] {
        var actions: [ChangesPopoverBatchAction] = []
        if !stagedSelection.isEmpty {
            actions.append(ChangesPopoverBatchAction(
                title: "Unstage Selected",
                action: { unstage(stagedSelection.files(in: changes.stagedFiles), from: .staged) }
            ))
        }
        actions.append(ChangesPopoverBatchAction(
            title: "Unstage All",
            action: { unstage(changes.stagedFiles, from: .staged) }
        ))
        return actions
    }

    private var unstagedBatchActions: [ChangesPopoverBatchAction] {
        var actions: [ChangesPopoverBatchAction] = []
        if !unstagedSelection.isEmpty {
            actions.append(ChangesPopoverBatchAction(
                title: "Stage Selected",
                action: { stage(unstagedSelection.files(in: changes.unstagedFiles), from: .unstaged) }
            ))
        }
        actions.append(ChangesPopoverBatchAction(
            title: "Stage All",
            action: { stage(changes.unstagedFiles, from: .unstaged) }
        ))
        return actions
    }

    private func selection(for side: ChangeSide) -> RepositoryChangesFileSelection {
        switch side {
        case .conflicted: conflictedSelection
        case .staged: stagedSelection
        case .unstaged: unstagedSelection
        }
    }

    private func files(for side: ChangeSide) -> [GitStatusFile] {
        switch side {
        case .conflicted: changes.conflictedFiles
        case .staged: changes.stagedFiles
        case .unstaged: changes.unstagedFiles
        }
    }

    private func handleFileClick(_ file: GitStatusFile, side: ChangeSide, event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        handleFileClick(
            file,
            side: side,
            kind: .from(command: flags.contains(.command), shift: flags.contains(.shift))
        )
    }

    private func handleFileClick(
        _ file: GitStatusFile,
        side: ChangeSide,
        kind: RepositoryChangesFileSelection.Click
    ) {
        let ids = files(for: side).map(\.id)
        switch side {
        case .conflicted:
            conflictedSelection.handleClick(id: file.id, ids: ids, kind: kind)
        case .staged:
            stagedSelection.handleClick(id: file.id, ids: ids, kind: kind)
        case .unstaged:
            unstagedSelection.handleClick(id: file.id, ids: ids, kind: kind)
        }
    }

    private func actionTargets(_ file: GitStatusFile, side: ChangeSide) -> [GitStatusFile] {
        selection(for: side).actionTargets(file, in: files(for: side))
    }

    private func rowID(_ file: GitStatusFile, side: ChangeSide) -> String {
        "\(side.rawValue):\(file.path)"
    }

    private func stage(_ files: [GitStatusFile], from side: ChangeSide) {
        removeFromSelection(files.map(\.id), side: side)
        onStage(files)
    }

    private func unstage(_ files: [GitStatusFile], from side: ChangeSide) {
        removeFromSelection(files.map(\.id), side: side)
        onUnstage(files)
    }

    private func removeFromSelection(_ ids: [String], side: ChangeSide) {
        switch side {
        case .conflicted:
            conflictedSelection.remove(ids: ids)
        case .staged:
            stagedSelection.remove(ids: ids)
        case .unstaged:
            unstagedSelection.remove(ids: ids)
        }
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

    private enum ChangeSide: String, Equatable, Hashable {
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

private struct ChangesPopoverBatchAction {
    let title: LocalizedStringResource
    let action: () -> Void
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
    let isSelected: Bool
    let content: Content

    @State private var isHovered = false

    init(isSelected: Bool, @ViewBuilder content: () -> Content) {
        self.isSelected = isSelected
        self.content = content()
    }

    var body: some View {
        content
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.scaled(34))
            .background(
                rowBackground,
                in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
            )
            .padding(.horizontal, UIMetrics.spacing2)
            .contentShape(Rectangle())
            .onHover { isHovered = $0 }
    }

    private var rowBackground: Color {
        if isSelected {
            return MuxyTheme.accentSoft
        }
        if isHovered {
            return MuxyTheme.hover
        }
        return .clear
    }
}
