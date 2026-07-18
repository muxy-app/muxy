import SwiftUI

struct SearchableListPicker<Item: Identifiable, RowContent: View>: View {
    let items: [Item]
    let filterKey: (Item) -> String
    let placeholder: String
    let emptyLabel: String
    let selectsRowOnTap: Bool
    let isSearchDisabled: Bool
    let searchFocusRequest: Int
    let onSearchChange: (String) -> Void
    let onEscape: () -> Void
    let onSelect: (Item) -> Void
    @ViewBuilder let row: (Item, Bool) -> RowContent

    @State private var searchText = ""
    @State private var highlightedIndex: Int?

    private var filteredItems: [Item] {
        guard !searchText.isEmpty else { return items }
        return items.filter { filterKey($0).localizedCaseInsensitiveContains(searchText) }
    }

    init(
        items: [Item],
        filterKey: @escaping (Item) -> String,
        placeholder: String,
        emptyLabel: String,
        selectsRowOnTap: Bool = true,
        isSearchDisabled: Bool = false,
        searchFocusRequest: Int = 0,
        onSearchChange: @escaping (String) -> Void = { _ in },
        onEscape: @escaping () -> Void = {},
        onSelect: @escaping (Item) -> Void,
        @ViewBuilder row: @escaping (Item, Bool) -> RowContent
    ) {
        self.items = items
        self.filterKey = filterKey
        self.placeholder = placeholder
        self.emptyLabel = emptyLabel
        self.selectsRowOnTap = selectsRowOnTap
        self.isSearchDisabled = isSearchDisabled
        self.searchFocusRequest = searchFocusRequest
        self.onSearchChange = onSearchChange
        self.onEscape = onEscape
        self.onSelect = onSelect
        self.row = row
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .font(.system(size: UIMetrics.fontBody))
                    .accessibilityHidden(true)
                PaletteSearchField(
                    text: $searchText,
                    placeholder: placeholder,
                    focusRequest: searchFocusRequest,
                    isEnabled: !isSearchDisabled,
                    fontSize: UIMetrics.fontBody,
                    onSubmit: { confirmSelection() },
                    onEscape: onEscape,
                    onArrowUp: { moveHighlight(-1) },
                    onArrowDown: { moveHighlight(1) },
                    onQueryChange: onSearchChange
                )
            }
            .padding(.horizontal, UIMetrics.spacing5)
            .padding(.vertical, UIMetrics.spacing4)

            Divider().overlay(MuxyTheme.border)

            if filteredItems.isEmpty {
                Text(emptyLabel)
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollViewReader { proxy in
                    ScrollView(.vertical, showsIndicators: false) {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(filteredItems.enumerated()), id: \.element.id) { index, item in
                                rowContent(item, isHighlighted: index == highlightedIndex)
                                    .id(item.id)
                            }
                        }
                        .padding(.vertical, UIMetrics.spacing2)
                    }
                    .onChange(of: highlightedIndex) { _, newIndex in
                        guard let newIndex, newIndex < filteredItems.count else { return }
                        proxy.scrollTo(filteredItems[newIndex].id, anchor: nil)
                    }
                }
            }
        }
        .background(MuxyTheme.bg)
        .onChange(of: searchText) { highlightedIndex = filteredItems.isEmpty ? nil : 0 }
    }

    @ViewBuilder
    private func rowContent(_ item: Item, isHighlighted: Bool) -> some View {
        if selectsRowOnTap {
            row(item, isHighlighted)
                .contentShape(Rectangle())
                .onTapGesture { onSelect(item) }
        } else {
            row(item, isHighlighted)
        }
    }

    private func moveHighlight(_ delta: Int) {
        let list = filteredItems
        guard !list.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : list.count - 1
            return
        }
        highlightedIndex = max(0, min(list.count - 1, current + delta))
    }

    private func confirmSelection() {
        let list = filteredItems
        guard let index = highlightedIndex, index < list.count else { return }
        onSelect(list[index])
    }
}
