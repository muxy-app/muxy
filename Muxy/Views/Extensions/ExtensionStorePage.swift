import SwiftUI

struct ExtensionStorePage: View {
    let store: ExtensionStore
    let onSelect: (String) -> Void

    @State private var query = ""
    @State private var committedQuery = ""
    @State private var sort: ExtensionSort = .all
    @State private var selectedCategory: String?
    @State private var categories: [ExtensionCategory] = []

    @State private var items: [ExtensionListing] = []
    @State private var page = 1
    @State private var hasNextPage = false
    @State private var phase: Phase = .loading
    @State private var isLoadingMore = false
    @State private var loadMoreTask: Task<Void, Never>?
    @State private var generation = 0

    private enum Phase: Equatable {
        case loading
        case loaded
        case empty
        case failed(String)
    }

    private struct LoadKey: Equatable {
        let query: String
        let sort: ExtensionSort
        let category: String?
    }

    private static let searchDebounce: Duration = .milliseconds(300)
    private static let pageSize = 24

    private let columns = [GridItem(.adaptive(minimum: 240, maximum: 360), spacing: 12)]

    private var loadKey: LoadKey {
        LoadKey(query: committedQuery, sort: sort, category: selectedCategory)
    }

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            Rectangle()
                .fill(MuxyTheme.border)
                .frame(height: 1)
            resultsArea
        }
        .task { await loadCategories() }
        .task(id: loadKey) { await reload() }
        .task(id: query) {
            try? await Task.sleep(for: Self.searchDebounce)
            guard !Task.isCancelled else { return }
            committedQuery = query
        }
        .onDisappear { loadMoreTask?.cancel() }
    }

    private var toolbar: some View {
        HStack(spacing: 10) {
            searchField
            sortMenu
            if !categories.isEmpty {
                categoryMenu
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
    }

    private var searchField: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgDim)
            TextField("Search extensions", text: $query)
                .textFieldStyle(.plain)
                .font(.system(size: 12))
            if !query.isEmpty {
                Button {
                    query = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .frame(maxWidth: 280)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }

    private var sortMenu: some View {
        Menu {
            ForEach(ExtensionSort.allCases, id: \.self) { option in
                Button {
                    sort = option
                } label: {
                    if sort == option {
                        Label(option.displayName, systemImage: "checkmark")
                    } else {
                        Text(option.displayName)
                    }
                }
            }
        } label: {
            ExtensionStoreMenuLabel(icon: "arrow.up.arrow.down", title: sort.displayName)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
    }

    private var categoryMenu: some View {
        Menu {
            Button {
                selectedCategory = nil
            } label: {
                if selectedCategory == nil {
                    Label("All Categories", systemImage: "checkmark")
                } else {
                    Text("All Categories")
                }
            }
            Divider()
            ForEach(categories) { category in
                Button {
                    selectedCategory = category.slug
                } label: {
                    if selectedCategory == category.slug {
                        Label("\(category.name) (\(category.count))", systemImage: "checkmark")
                    } else {
                        Text("\(category.name) (\(category.count))")
                    }
                }
            }
        } label: {
            ExtensionStoreMenuLabel(icon: "line.3.horizontal.decrease", title: categoryLabel)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
    }

    private var categoryLabel: String {
        guard let selectedCategory else { return "All Categories" }
        return categories.first { $0.slug == selectedCategory }?.name ?? selectedCategory
    }

    @ViewBuilder
    private var resultsArea: some View {
        switch phase {
        case .loading:
            loadingState
        case .empty:
            emptyState
        case let .failed(message):
            failedState(message)
        case .loaded:
            grid
        }
    }

    private var installedIDs: Set<String> {
        Set(store.statuses.map(\.id))
    }

    private var grid: some View {
        let installed = installedIDs
        return ScrollView {
            LazyVGrid(columns: columns, spacing: 12) {
                ForEach(items) { listing in
                    ExtensionStoreCard(
                        listing: listing,
                        isInstalled: installed.contains(listing.name),
                        onSelect: { onSelect(listing.name) }
                    )
                    .onAppear {
                        if listing.id == items.last?.id {
                            loadMoreIfNeeded()
                        }
                    }
                }
            }
            .padding(20)
            if isLoadingMore {
                ProgressView()
                    .controlSize(.small)
                    .padding(.bottom, 20)
            }
        }
    }

    private var loadingState: some View {
        HStack(spacing: 10) {
            ProgressView().controlSize(.small)
            Text("Loading extensions…")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 32))
                .foregroundStyle(MuxyTheme.fgDim)
            Text("No extensions found")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text(committedQuery.isEmpty ? "Check back soon for new extensions." : "Try a different search or category.")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func failedState(_ message: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 28))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(message)
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            Button {
                Task { await reload() }
            } label: {
                Text("Retry")
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.accent)
            }
            .buttonStyle(.plain)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(40)
    }

    private func reload() async {
        loadMoreTask?.cancel()
        isLoadingMore = false
        generation += 1
        let token = generation
        page = 1
        if items.isEmpty {
            phase = .loading
        }
        do {
            let result = try await ExtensionMarketplaceService.shared.list(query: makeQuery(page: 1))
            guard token == generation, !Task.isCancelled else { return }
            items = result.items
            hasNextPage = result.hasNextPage
            phase = result.items.isEmpty ? .empty : .loaded
        } catch {
            guard token == generation, !Task.isCancelled else { return }
            items = []
            phase = .failed(message(for: error))
        }
    }

    private func loadMoreIfNeeded() {
        guard hasNextPage, !isLoadingMore, phase == .loaded else { return }
        isLoadingMore = true
        let token = generation
        let next = page + 1
        let request = makeQuery(page: next)
        loadMoreTask?.cancel()
        loadMoreTask = Task {
            defer { isLoadingMore = false }
            do {
                let result = try await ExtensionMarketplaceService.shared.list(query: request)
                guard token == generation, !Task.isCancelled else { return }
                items.append(contentsOf: result.items)
                hasNextPage = result.hasNextPage
                page = next
            } catch {
                guard token == generation, !Task.isCancelled else { return }
                hasNextPage = false
            }
        }
    }

    private func loadCategories() async {
        do {
            let result = try await ExtensionMarketplaceService.shared.categories()
            guard !Task.isCancelled else { return }
            categories = result
        } catch {
            categories = []
        }
    }

    private func makeQuery(page: Int) -> ExtensionCatalogQuery {
        ExtensionCatalogQuery(
            search: committedQuery.isEmpty ? nil : committedQuery,
            sort: sort,
            category: selectedCategory,
            official: nil,
            page: page,
            perPage: Self.pageSize
        )
    }

    private func message(for error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }
}

private struct ExtensionStoreMenuLabel: View {
    let icon: String
    let title: String

    var body: some View {
        HStack(spacing: 5) {
            Image(systemName: icon)
                .font(.system(size: 10, weight: .medium))
            Text(title)
                .font(.system(size: 12))
            Image(systemName: "chevron.down")
                .font(.system(size: 9, weight: .semibold))
        }
        .foregroundStyle(MuxyTheme.fgMuted)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }
}

private struct ExtensionStoreCard: View {
    let listing: ExtensionListing
    let isInstalled: Bool
    let onSelect: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: onSelect) {
            VStack(alignment: .leading, spacing: 10) {
                header
                if let description = listing.description, !description.isEmpty {
                    Text(description)
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Spacer(minLength: 0)
                footer
            }
            .padding(14)
            .frame(maxWidth: .infinity, minHeight: 132, alignment: .topLeading)
            .background(hovered ? MuxyTheme.hover : MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 10))
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .stroke(MuxyTheme.border, lineWidth: 1)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }

    private var header: some View {
        HStack(alignment: .top, spacing: 10) {
            ExtensionStoreCardIcon(urlString: listing.iconURL)
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(listing.name)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                    if listing.official {
                        ExtensionStoreBadge(label: "OFFICIAL", color: MuxyTheme.accent)
                    }
                }
                if let author = listing.author?.name, !author.isEmpty {
                    Text("by \(author)")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            if isInstalled {
                ExtensionStoreBadge(label: "Installed", color: MuxyTheme.diffAddFg)
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 10) {
            Label("\(listing.downloads)", systemImage: "arrow.down.circle")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgDim)
            Text("v\(listing.version)")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgDim)
            Spacer(minLength: 0)
        }
    }
}

private struct ExtensionStoreBadge: View {
    let label: String
    let color: Color

    var body: some View {
        Text(label)
            .font(.system(size: 9, weight: .semibold))
            .foregroundStyle(color)
            .padding(.horizontal, 5)
            .padding(.vertical, 2)
            .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
            .lineLimit(1)
            .fixedSize()
    }
}

private struct ExtensionStoreCardIcon: View {
    let urlString: String?

    var body: some View {
        ExtensionRemoteIconView(urlString: urlString, placeholderSize: 17)
            .frame(width: 40, height: 40)
            .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: 9))
            .overlay(
                RoundedRectangle(cornerRadius: 9)
                    .stroke(MuxyTheme.border, lineWidth: 1)
            )
    }
}
