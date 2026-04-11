import SwiftUI

struct PopoverPicker<Item: Identifiable, RowContent: View>: View {
    let items: [Item]
    let filterKey: (Item) -> String
    let searchPlaceholder: String
    let emptyLabel: String
    let footerTitle: String?
    let footerIcon: String?
    let onFooterAction: (() -> Void)?
    let onSelect: (Item) -> Void
    @ViewBuilder let row: (Item, Bool) -> RowContent

    init(
        items: [Item],
        filterKey: @escaping (Item) -> String,
        searchPlaceholder: String,
        emptyLabel: String,
        footerTitle: String? = nil,
        footerIcon: String? = nil,
        onFooterAction: (() -> Void)? = nil,
        onSelect: @escaping (Item) -> Void,
        @ViewBuilder row: @escaping (Item, Bool) -> RowContent
    ) {
        self.items = items
        self.filterKey = filterKey
        self.searchPlaceholder = searchPlaceholder
        self.emptyLabel = emptyLabel
        self.footerTitle = footerTitle
        self.footerIcon = footerIcon
        self.onFooterAction = onFooterAction
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
            if let footerTitle, let onFooterAction {
                Divider().overlay(MuxyTheme.border.opacity(0.55))
                footerButton(title: footerTitle, icon: footerIcon, action: onFooterAction)
            }
        }
        .frame(width: 300, height: 420)
        .background(MuxyTheme.bg)
    }

    private func footerButton(title: String, icon: String?, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let icon {
                    Image(systemName: icon)
                        .font(.system(size: 12))
                }
                Text(title)
                    .font(.system(size: 12, weight: .medium))
                Spacer()
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
