import SwiftUI

struct QuickOpenOverlay: View {
    let projectPath: String
    let onSelect: (String) -> Void
    let onDismiss: () -> Void

    @State private var query = ""
    @State private var results: [FileSearchResult] = []
    @State private var fileIndex: [FileSearchResult] = []
    @State private var highlightedIndex: Int? = 0
    @State private var isIndexing = true
    @FocusState private var searchFieldFocused: Bool
    @State private var indexTask: Task<Void, Never>?
    @State private var searchTask: Task<Void, Never>?

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture { onDismiss() }

            VStack(spacing: 0) {
                searchField
                Divider().overlay(MuxyTheme.border)
                resultsList
            }
            .frame(width: 500, height: 380)
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(MuxyTheme.border, lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: 20, y: 8)
            .padding(.top, 60)
            .frame(maxHeight: .infinity, alignment: .top)
        }
        .onAppear {
            searchFieldFocused = true
            loadInitialResults()
        }
        .onDisappear {
            indexTask?.cancel()
            searchTask?.cancel()
        }
        .onKeyPress(.escape) {
            onDismiss()
            return .handled
        }
    }

    private var searchField: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(MuxyTheme.fgMuted)
                .font(.system(size: 13))
            ZStack(alignment: .leading) {
                if query.isEmpty {
                    Text("Search files by name...")
                        .font(.system(size: 13))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                TextField("", text: $query)
                    .textFieldStyle(.plain)
                    .font(.system(size: 13))
                    .foregroundStyle(MuxyTheme.fg)
                    .focused($searchFieldFocused)
                    .onSubmit { confirmSelection() }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .onChange(of: query) {
            performSearch()
        }
        .onKeyPress(.upArrow) {
            moveHighlight(-1)
            return .handled
        }
        .onKeyPress(.downArrow) {
            moveHighlight(1)
            return .handled
        }
    }

    private var resultsList: some View {
        Group {
            if isIndexing {
                VStack {
                    Spacer()
                    ProgressView()
                        .controlSize(.small)
                    Text("Indexing files...")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .padding(.top, 4)
                    Spacer()
                }
            } else if results.isEmpty {
                VStack {
                    Spacer()
                    Text(query.isEmpty ? "No files found" : "No matching files")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Spacer()
                }
            } else {
                ScrollViewReader { proxy in
                    ScrollView(.vertical, showsIndicators: true) {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(results.enumerated()), id: \.element.id) { index, result in
                                FileResultRow(
                                    result: result,
                                    isHighlighted: index == highlightedIndex,
                                    onHover: { highlightedIndex = index }
                                )
                                .contentShape(Rectangle())
                                .onTapGesture { onSelect(result.absolutePath) }
                                .id(result.id)
                            }
                        }
                    }
                    .onChange(of: highlightedIndex) { _, newIndex in
                        guard let newIndex, newIndex < results.count else { return }
                        proxy.scrollTo(results[newIndex].id, anchor: nil)
                    }
                }
            }
        }
        .frame(maxHeight: .infinity)
    }

    private func loadInitialResults() {
        indexTask = Task {
            let index = await FileSearchService.shared.getIndex(projectPath: projectPath)
            guard !Task.isCancelled else { return }
            await MainActor.run {
                fileIndex = index
                results = Array(index.prefix(200))
                isIndexing = false
                highlightedIndex = results.isEmpty ? nil : 0
            }
        }
    }

    private func performSearch() {
        searchTask?.cancel()
        let currentIndex = fileIndex
        guard !currentIndex.isEmpty else { return }
        let currentQuery = query
        searchTask = Task.detached(priority: .userInitiated) {
            let searchResults = FileSearchService.search(query: currentQuery, in: currentIndex)
            guard !Task.isCancelled else { return }
            await MainActor.run {
                results = searchResults
                highlightedIndex = searchResults.isEmpty ? nil : 0
            }
        }
    }

    private func moveHighlight(_ delta: Int) {
        guard !results.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : results.count - 1
            return
        }
        highlightedIndex = max(0, min(results.count - 1, current + delta))
    }

    private func confirmSelection() {
        guard let index = highlightedIndex, index < results.count else { return }
        onSelect(results[index].absolutePath)
    }
}

private struct FileResultRow: View {
    let result: FileSearchResult
    let isHighlighted: Bool
    let onHover: () -> Void
    @State private var hovered = false

    private var fileIcon: String {
        let ext = URL(fileURLWithPath: result.absolutePath).pathExtension.lowercased()
        switch ext {
        case "swift": return "swift"
        case "js",
             "jsx",
             "mjs": return "j.square"
        case "ts",
             "tsx",
             "mts": return "t.square"
        case "py": return "p.square"
        case "json": return "curlybraces"
        case "html",
             "htm": return "chevron.left.forwardslash.chevron.right"
        case "css",
             "scss": return "paintbrush"
        case "md",
             "markdown": return "doc.richtext"
        case "yaml",
             "yml",
             "toml": return "gearshape"
        case "sh",
             "bash",
             "zsh": return "terminal"
        default: return "doc.text"
        }
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: fileIcon)
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 16)
            VStack(alignment: .leading, spacing: 1) {
                Text(result.fileName)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                Text(result.relativePath)
                    .font(.system(size: 10))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .onHover { isHovered in
            hovered = isHovered
            if isHovered { onHover() }
        }
    }
}
