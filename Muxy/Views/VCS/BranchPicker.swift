import SwiftUI

struct BranchPicker: View {
    let currentBranch: String?
    let branches: [String]
    let isLoading: Bool
    let onSelect: (String) -> Void
    let onRefresh: () -> Void
    let onCreateBranch: (() -> Void)?
    let onDeleteBranch: ((String) -> Void)?
    @State private var showPopover = false

    private var branchItems: [BranchItem] {
        branches.map { BranchItem(name: $0) }
    }

    var body: some View {
        Button {
            onRefresh()
            showPopover.toggle()
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "arrow.triangle.branch")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                Text(currentBranch ?? "detached")
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .frame(maxWidth: 120, alignment: .leading)
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
            .foregroundStyle(MuxyTheme.fg.opacity(0.85))
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.scaled(3))
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .buttonStyle(.plain)
        .help(currentBranch ?? "detached")
        .accessibilityLabel("Branch: \(currentBranch ?? "detached")")
        .accessibilityHint("Opens branch picker")
        .popover(isPresented: $showPopover, arrowEdge: .top) {
            BranchPickerContent(
                currentBranch: currentBranch,
                branches: branches,
                isLoading: isLoading,
                fixedSize: true,
                onSelect: { name in
                    showPopover = false
                    onSelect(name)
                },
                onCreateBranch: onCreateBranch.map { action in
                    {
                        showPopover = false
                        action()
                    }
                },
                onDeleteBranch: onDeleteBranch.map { action in
                    { name in
                        showPopover = false
                        action(name)
                    }
                }
            )
        }
    }
}

struct BranchPickerContent: View {
    let currentBranch: String?
    let branches: [String]
    let isLoading: Bool
    var fixedSize: Bool = false
    let onSelect: (String) -> Void
    let onCreateBranch: (() -> Void)?
    let onDeleteBranch: ((String) -> Void)?

    private var items: [BranchItem] { branches.map { BranchItem(name: $0) } }

    var body: some View {
        PopoverPicker(
            items: items,
            filterKey: \.name,
            searchPlaceholder: "Search branches…",
            emptyLabel: isLoading ? "Loading…" : "No branches found",
            footerActions: onCreateBranch.map { action in
                [
                    PopoverFooterAction(
                        title: "New Branch…",
                        icon: "plus.square.dashed",
                        action: action
                    ),
                ]
            } ?? [],
            fixedSize: fixedSize,
            onSelect: { item in onSelect(item.name) },
            row: { item, isHighlighted in
                BranchRow(
                    name: item.name,
                    isActive: item.name == currentBranch,
                    isHighlighted: isHighlighted
                )
                .padding(.horizontal, UIMetrics.spacing3)
                .padding(.vertical, UIMetrics.spacing1)
                .contextMenu {
                    if let onDeleteBranch, item.name != currentBranch {
                        Button("Delete Branch", role: .destructive) {
                            onDeleteBranch(item.name)
                        }
                    }
                }
            }
        )
    }
}

private struct BranchItem: Identifiable {
    let name: String
    var id: String { name }
}

private struct BranchRow: View {
    let name: String
    let isActive: Bool
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        HStack(spacing: UIMetrics.spacing5) {
            Circle()
                .fill(isActive ? MuxyTheme.accent : MuxyTheme.fgDim.opacity(0.35))
                .frame(width: UIMetrics.scaled(7), height: UIMetrics.scaled(7))
                .frame(width: UIMetrics.scaled(10))

            Text(name)
                .font(.system(size: UIMetrics.fontBody, weight: isActive ? .semibold : .medium, design: .monospaced))
                .foregroundStyle(isActive ? MuxyTheme.fg : MuxyTheme.fg.opacity(0.9))
                .lineLimit(1)
                .truncationMode(.middle)

            Spacer(minLength: 4)

            if isActive {
                Image(systemName: "checkmark")
                    .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                    .foregroundStyle(MuxyTheme.accent)
            }
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.scaled(7))
        .background(rowBackground, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .onHover { hovered = $0 }
    }

    private var rowBackground: AnyShapeStyle {
        if isActive { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if isHighlighted { return AnyShapeStyle(MuxyTheme.surface) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }
}
