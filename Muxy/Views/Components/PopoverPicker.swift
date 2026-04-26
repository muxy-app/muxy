import SwiftUI

struct PopoverFooterAction: Identifiable {
    let id: String
    let title: String
    let icon: String?
    let isBusy: Bool
    let action: () -> Void

    init(
        id: String? = nil,
        title: String,
        icon: String? = nil,
        isBusy: Bool = false,
        action: @escaping () -> Void
    ) {
        self.id = id ?? "\(title)|\(icon ?? "")"
        self.title = title
        self.icon = icon
        self.isBusy = isBusy
        self.action = action
    }
}

struct PopoverPicker<Item: Identifiable, RowContent: View>: View {
    let items: [Item]
    let filterKey: (Item) -> String
    let searchPlaceholder: String
    let emptyLabel: String
    let footerActions: [PopoverFooterAction]
    let fixedSize: Bool
    let onSelect: (Item) -> Void
    @ViewBuilder let row: (Item, Bool) -> RowContent

    init(
        items: [Item],
        filterKey: @escaping (Item) -> String,
        searchPlaceholder: String,
        emptyLabel: String,
        footerActions: [PopoverFooterAction] = [],
        fixedSize: Bool = true,
        onSelect: @escaping (Item) -> Void,
        @ViewBuilder row: @escaping (Item, Bool) -> RowContent
    ) {
        self.items = items
        self.filterKey = filterKey
        self.searchPlaceholder = searchPlaceholder
        self.emptyLabel = emptyLabel
        self.footerActions = footerActions
        self.fixedSize = fixedSize
        self.onSelect = onSelect
        self.row = row
    }

    var body: some View {
        VStack(spacing: 0) {
            SearchableListPicker(
                items: items,
                filterKey: filterKey,
                placeholder: searchPlaceholder,
                emptyLabel: emptyLabel,
                onSelect: onSelect,
                row: row
            )
            if !footerActions.isEmpty {
                Divider().overlay(MuxyTheme.border.opacity(0.55))
                VStack(spacing: 0) {
                    ForEach(footerActions) { footerAction in
                        footerButton(
                            title: footerAction.title,
                            icon: footerAction.icon,
                            isBusy: footerAction.isBusy,
                            action: footerAction.action
                        )
                    }
                }
            }
        }
        .frame(width: fixedSize ? 300 : nil, height: fixedSize ? 420 : nil)
        .frame(maxWidth: fixedSize ? nil : .infinity, maxHeight: fixedSize ? nil : .infinity)
        .background(MuxyTheme.bg)
    }

    private func footerButton(
        title: String,
        icon: String?,
        isBusy: Bool,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let icon {
                    Image(systemName: icon)
                        .font(.system(size: 12))
                }
                Text(title)
                    .font(.system(size: 12, weight: .medium))
                Spacer()
                ProgressView()
                    .controlSize(.small)
                    .scaleEffect(0.7)
                    .frame(width: 12, height: 12)
                    .opacity(isBusy ? 1 : 0)
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(isBusy)
    }
}
