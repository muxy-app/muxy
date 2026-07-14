import SwiftUI

struct TabFocusedBranchPopover: View {
    let summary: GitRepositorySummary
    let branches: [String]
    let isLoadingBranches: Bool
    let isSwitching: Bool
    let isRepositoryInteractionDisabled: Bool
    let onSwitch: (String) -> Void

    private struct BranchItem: Identifiable {
        let name: String
        var id: String { name }
    }

    private var items: [BranchItem] {
        branches.map { BranchItem(name: $0) }
    }

    var body: some View {
        branchPicker
            .frame(width: UIMetrics.scaled(320), height: UIMetrics.scaled(420))
            .background(MuxyTheme.bg)
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
            .disabled(isSwitching || isRepositoryInteractionDisabled)
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
}
