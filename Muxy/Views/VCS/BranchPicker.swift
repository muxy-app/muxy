import SwiftUI

struct BranchPicker: View {
    let currentBranch: String?
    let branches: [String]
    let isLoading: Bool
    let onSelect: (String) -> Void
    let onRefresh: () -> Void
    @State private var showPopover = false

    private var branchItems: [BranchItem] {
        branches.map { BranchItem(name: $0) }
    }

    var body: some View {
        Button {
            onRefresh()
            showPopover.toggle()
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "arrow.triangle.branch")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)

                Text(currentBranch ?? "detached")
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)

                Image(systemName: "chevron.down")
                    .font(.system(size: 8, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
        }
        .buttonStyle(.plain)
        .popover(isPresented: $showPopover, arrowEdge: .bottom) {
            if isLoading {
                ProgressView()
                    .frame(width: 240, height: 60)
            } else {
                SearchableListPicker(
                    items: branchItems,
                    filterKey: \.name,
                    placeholder: "Filter branches",
                    emptyLabel: "No branches found",
                    onSelect: { item in
                        showPopover = false
                        onSelect(item.name)
                    },
                    row: { item, isHighlighted in
                        BranchRow(
                            name: item.name,
                            isActive: item.name == currentBranch,
                            isHighlighted: isHighlighted
                        )
                    }
                )
                .frame(width: 240, height: 300)
            }
        }
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
        HStack(spacing: 6) {
            Text(name)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(isActive ? MuxyTheme.accent : MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)

            Spacer(minLength: 0)

            if isActive {
                Image(systemName: "checkmark")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(MuxyTheme.accent)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(isHighlighted ? MuxyTheme.surface : (hovered ? MuxyTheme.hover : .clear))
        .onHover { hovered = $0 }
    }
}
