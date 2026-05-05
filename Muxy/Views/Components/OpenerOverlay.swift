import AppKit
import SwiftUI

struct OpenerOverlay: View {
    let items: [OpenerItem]
    let recents: [OpenerItem]
    let activeWorktreeKey: WorktreeKey?
    let onSelect: (OpenerItem) -> Void
    let onDismiss: () -> Void

    @State private var query = ""
    @State private var enabledCategories: Set<OpenerCategory> = OpenerPreferences.enabledCategories
    @State private var highlightedIndex: Int? = 0

    private var filteredItems: [OpenerItem] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        let scoped = items.filter { enabledCategories.contains($0.category) }
        guard !trimmed.isEmpty else { return scoped }
        return scoped.filter { $0.searchKey.localizedCaseInsensitiveContains(trimmed) }
    }

    private var visibleRecents: [OpenerItem] {
        guard query.trimmingCharacters(in: .whitespaces).isEmpty else { return [] }
        return recents.filter { enabledCategories.contains($0.category) }
    }

    private var displayList: [OpenerItem] {
        let recentIDs = Set(visibleRecents.map(\.id))
        return visibleRecents + filteredItems.filter { !recentIDs.contains($0.id) }
    }

    private var firstNonRecentIndex: Int? {
        let recentCount = visibleRecents.count
        return recentCount < displayList.count ? recentCount : nil
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture { onDismiss() }

            VStack(spacing: 0) {
                searchField
                Divider().overlay(MuxyTheme.border)
                categoryChips
                Divider().overlay(MuxyTheme.border)
                resultsList
            }
            .frame(width: 560, height: 460)
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(MuxyTheme.border, lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: 20, y: 8)
            .padding(.top, 60)
            .frame(maxHeight: .infinity, alignment: .top)
            .accessibilityAddTraits(.isModal)
        }
        .onAppear {
            highlightedIndex = displayList.isEmpty ? nil : 0
        }
        .onChange(of: query) {
            highlightedIndex = displayList.isEmpty ? nil : 0
        }
        .onChange(of: enabledCategories) {
            OpenerPreferences.enabledCategories = enabledCategories
            highlightedIndex = displayList.isEmpty ? nil : 0
        }
        .onChange(of: items.count) {
            highlightedIndex = displayList.isEmpty ? nil : 0
        }
    }

    private var searchField: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(MuxyTheme.fgMuted)
                .font(.system(size: 13))
                .accessibilityHidden(true)
            PaletteSearchField(
                text: $query,
                placeholder: "Search projects, worktrees, layouts, branches, tabs...",
                onSubmit: { confirmSelection() },
                onEscape: { onDismiss() },
                onArrowUp: { moveHighlight(-1) },
                onArrowDown: { moveHighlight(1) }
            )
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }

    private var categoryChips: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(OpenerCategory.allCases) { category in
                    OpenerCategoryChip(
                        category: category,
                        isOn: enabledCategories.contains(category),
                        action: { toggleCategory(category) }
                    )
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
        }
    }

    private var resultsList: some View {
        Group {
            if displayList.isEmpty {
                VStack {
                    Spacer()
                    Text(query.isEmpty ? "No items" : "No matches")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Spacer()
                }
            } else {
                ScrollViewReader { proxy in
                    ScrollView(.vertical, showsIndicators: true) {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(displayList.enumerated()), id: \.element.id) { index, item in
                                if index == 0, !visibleRecents.isEmpty {
                                    OpenerSectionHeader(title: "Recent")
                                }
                                if let firstOther = firstNonRecentIndex, index == firstOther {
                                    OpenerSectionHeader(title: "All")
                                }
                                OpenerRow(
                                    item: item,
                                    isHighlighted: index == highlightedIndex,
                                    isActive: isActive(item)
                                )
                                .contentShape(Rectangle())
                                .onTapGesture { onSelect(item) }
                                .id(item.id)
                            }
                        }
                    }
                    .onChange(of: highlightedIndex) { _, newIndex in
                        guard let newIndex, newIndex < displayList.count else { return }
                        proxy.scrollTo(displayList[newIndex].id, anchor: nil)
                    }
                }
            }
        }
        .frame(maxHeight: .infinity)
    }

    private func toggleCategory(_ category: OpenerCategory) {
        var current = enabledCategories
        if current.contains(category), current.count > 1 {
            current.remove(category)
        } else {
            current.insert(category)
        }
        enabledCategories = current
    }

    private func moveHighlight(_ delta: Int) {
        guard !displayList.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : displayList.count - 1
            return
        }
        highlightedIndex = max(0, min(displayList.count - 1, current + delta))
    }

    private func confirmSelection() {
        guard let index = highlightedIndex, index < displayList.count else { return }
        onSelect(displayList[index])
    }

    private func isActive(_ item: OpenerItem) -> Bool {
        guard let activeKey = activeWorktreeKey else { return false }
        if case let .worktree(wt) = item {
            return WorktreeKey(projectID: wt.projectID, worktreeID: wt.worktreeID) == activeKey
        }
        return false
    }
}

private struct OpenerCategoryChip: View {
    let category: OpenerCategory
    let isOn: Bool
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 5) {
                Image(systemName: category.symbol)
                    .font(.system(size: 9, weight: .semibold))
                Text(category.label)
                    .font(.system(size: 11, weight: .medium))
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 4)
            .foregroundStyle(isOn ? MuxyTheme.fg : MuxyTheme.fgMuted)
            .background(isOn ? MuxyTheme.surface : (hovered ? MuxyTheme.hover : .clear), in: Capsule())
            .overlay(Capsule().stroke(isOn ? MuxyTheme.accent.opacity(0.6) : MuxyTheme.border, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(isOn ? "Hide \(category.label)" : "Show \(category.label)")
    }
}

private struct OpenerSectionHeader: View {
    let title: String

    var body: some View {
        HStack {
            Text(title.uppercased())
                .font(.system(size: 9, weight: .bold))
                .tracking(0.6)
                .foregroundStyle(MuxyTheme.fgDim)
            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.top, 8)
        .padding(.bottom, 3)
    }
}

private struct OpenerRow: View {
    let item: OpenerItem
    let isHighlighted: Bool
    let isActive: Bool
    @State private var hovered = false

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: item.category.symbol)
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 16, alignment: .center)
            Text(item.title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
            if case let .worktree(wt) = item, wt.isPrimary {
                Text("PRIMARY")
                    .font(.system(size: 8, weight: .bold))
                    .tracking(0.5)
                    .foregroundStyle(MuxyTheme.fgDim)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 1)
                    .background(MuxyTheme.surface, in: Capsule())
            }
            if let subtitle = item.subtitle {
                Text(subtitle)
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer(minLength: 4)
            if isActive {
                Image(systemName: "checkmark")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(MuxyTheme.accent)
            }
        }
        .frame(height: 28)
        .padding(.horizontal, 12)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .onHover { hovered = $0 }
    }
}
