import SwiftUI

struct TabFocusedBranchPopover: View {
    let summary: GitRepositorySummary
    let branches: [String]
    let isLoadingBranches: Bool
    let isRefreshing: Bool
    let isSwitching: Bool
    let isWorktreeRemovalInProgress: Bool
    let isRepositoryInteractionDisabled: Bool
    let worktreeRemovalState: RepositoryToolbarPresentation.WorktreeRemovalState
    let worktreeRemovalHelp: String?
    let onSwitch: (String) -> Void
    let onRefresh: () -> Void
    let onRemoveWorktree: () -> Void

    @State private var isRemoveWorktreeHovered = false

    private struct BranchItem: Identifiable {
        let name: String
        var id: String { name }
    }

    private var items: [BranchItem] {
        branches.map { BranchItem(name: $0) }
    }

    var body: some View {
        VStack(spacing: 0) {
            summaryHeader
            Divider().overlay(MuxyTheme.border)
            branchPicker
            worktreeRemovalFooter
        }
        .frame(width: UIMetrics.scaled(320), height: UIMetrics.scaled(420))
        .background(MuxyTheme.bg)
    }

    private var summaryHeader: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            HStack(spacing: UIMetrics.spacing4) {
                Image(systemName: "arrow.triangle.branch")
                    .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
                VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                    Text(summary.displayBranch)
                        .font(.system(size: UIMetrics.fontBody, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Text(summary.isDetached ? "Detached HEAD" : "Current branch")
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                Spacer(minLength: UIMetrics.spacing3)
                statusBadge
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
                .disabled(isRefreshing || isWorktreeRemovalInProgress || isRepositoryInteractionDisabled)
                .help("Refresh repository status")
            }
        }
        .padding(UIMetrics.spacing6)
    }

    private var statusBadge: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Circle()
                .fill(summary.isDirty ? MuxyTheme.warning : MuxyTheme.diffAddFg)
                .frame(width: UIMetrics.spacing2, height: UIMetrics.spacing2)
            Text(summary.isDirty ? "Dirty" : "Clean")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
        }
        .foregroundStyle(summary.isDirty ? MuxyTheme.warning : MuxyTheme.diffAddFg)
        .padding(.horizontal, UIMetrics.spacing3)
        .padding(.vertical, UIMetrics.spacing2)
        .background(MuxyTheme.surface, in: Capsule())
    }

    @ViewBuilder
    private var branchPicker: some View {
        if isLoadingBranches, branches.isEmpty {
            ProgressView()
                .controlSize(.small)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            SearchableListPicker(
                items: items,
                filterKey: { $0.name },
                placeholder: "Search branches…",
                emptyLabel: "No branches",
                onSelect: { onSwitch($0.name) },
                row: { item, isHighlighted in
                    row(item, isHighlighted: isHighlighted)
                }
            )
            .disabled(isSwitching || isWorktreeRemovalInProgress || isRepositoryInteractionDisabled)
        }
    }

    private func row(_ item: BranchItem, isHighlighted: Bool) -> some View {
        let selected = item.name == summary.branch
        return HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "arrow.triangle.branch")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(selected ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(14))
            Text(item.name)
                .font(.system(size: UIMetrics.fontBody, weight: selected ? .semibold : .regular, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: UIMetrics.spacing1)
            if selected {
                Image(systemName: "checkmark")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
            }
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.scaled(7))
        .background(
            rowBackground(selected: selected, isHighlighted: isHighlighted),
            in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
        )
        .padding(.horizontal, UIMetrics.spacing3)
        .padding(.vertical, UIMetrics.spacing1)
        .contentShape(Rectangle())
    }

    private func rowBackground(selected: Bool, isHighlighted: Bool) -> AnyShapeStyle {
        if selected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if isHighlighted { return AnyShapeStyle(MuxyTheme.surface) }
        return AnyShapeStyle(Color.clear)
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
                .foregroundStyle(worktreeRemovalForeground)
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
        worktreeRemovalState != .available
            || isSwitching
            || isWorktreeRemovalInProgress
            || isRepositoryInteractionDisabled
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

    private var worktreeRemovalForeground: Color {
        isWorktreeRemovalDisabled ? MuxyTheme.fgMuted : MuxyTheme.diffRemoveFg
    }
}
