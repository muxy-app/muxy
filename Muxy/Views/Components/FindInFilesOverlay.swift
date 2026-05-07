import SwiftUI

struct FindInFilesOverlay: View {
    let projectPath: String
    let onSelect: (TextSearchMatch) -> Void
    let onDismiss: () -> Void

    @State private var query = ""
    @State private var groups: [FileMatchGroup] = []
    @State private var highlightedMatchID: String?
    @State private var isSearching = false
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
            .frame(width: UIMetrics.scaled(640), height: UIMetrics.scaled(460))
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusXL))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusXL).stroke(MuxyTheme.border, lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: UIMetrics.scaled(20), y: UIMetrics.scaled(8))
            .padding(.top, UIMetrics.scaled(60))
            .frame(maxHeight: .infinity, alignment: .top)
            .accessibilityAddTraits(.isModal)
        }
        .onAppear { performSearch(debounce: false) }
        .onDisappear { searchTask?.cancel() }
    }

    private var searchField: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(MuxyTheme.fgMuted)
                .font(.system(size: UIMetrics.fontEmphasis))
                .accessibilityHidden(true)
            PaletteSearchField(
                text: $query,
                placeholder: "Search text in files...",
                onSubmit: { confirmSelection() },
                onEscape: { onDismiss() },
                onArrowUp: { moveHighlight(-1) },
                onArrowDown: { moveHighlight(1) }
            )
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing5)
        .onChange(of: query) { performSearch() }
    }

    @ViewBuilder
    private var resultsList: some View {
        if groups.isEmpty, !isSearching {
            VStack {
                Spacer()
                Text(query.trimmingCharacters(in: .whitespaces).count < TextSearchService.minQueryLength
                    ? "Type at least \(TextSearchService.minQueryLength) characters"
                    : "No matches found")
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Spacer()
            }
            .frame(maxHeight: .infinity)
        } else {
            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: true) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(groups) { group in
                            FileGroupHeader(group: group)
                            ForEach(group.matches) { match in
                                MatchRow(
                                    match: match,
                                    isHighlighted: match.id == highlightedMatchID
                                )
                                .contentShape(Rectangle())
                                .onTapGesture { onSelect(match) }
                                .id(match.id)
                            }
                        }
                    }
                    .padding(.vertical, UIMetrics.spacing2)
                }
                .onChange(of: highlightedMatchID) { _, newID in
                    guard let newID else { return }
                    withAnimation(.linear(duration: 0.05)) {
                        proxy.scrollTo(newID, anchor: nil)
                    }
                }
            }
        }
    }

    private func performSearch(debounce: Bool = true) {
        searchTask?.cancel()
        isSearching = true
        let currentQuery = query

        searchTask = Task {
            if debounce {
                try? await Task.sleep(for: .milliseconds(50))
                guard !Task.isCancelled else { return }
            }

            let found = await TextSearchService.search(query: currentQuery, in: projectPath)
            guard !Task.isCancelled else { return }

            await MainActor.run {
                groups = FileMatchGroup.group(found)
                highlightedMatchID = groups.first?.matches.first?.id
                isSearching = false
            }
        }
    }

    private func moveHighlight(_ delta: Int) {
        let flat = groups.flatMap(\.matches)
        guard !flat.isEmpty else { return }
        guard let current = highlightedMatchID,
              let index = flat.firstIndex(where: { $0.id == current })
        else {
            highlightedMatchID = (delta > 0 ? flat.first : flat.last)?.id
            return
        }
        let next = max(0, min(flat.count - 1, index + delta))
        highlightedMatchID = flat[next].id
    }

    private func confirmSelection() {
        guard let id = highlightedMatchID,
              let match = groups.flatMap(\.matches).first(where: { $0.id == id })
        else { return }
        onSelect(match)
    }
}

struct FileMatchGroup: Identifiable {
    let id: String
    let absolutePath: String
    let relativePath: String
    let matches: [TextSearchMatch]

    static func group(_ matches: [TextSearchMatch]) -> [FileMatchGroup] {
        var order: [String] = []
        var bucket: [String: [TextSearchMatch]] = [:]
        for match in matches {
            if bucket[match.absolutePath] == nil {
                order.append(match.absolutePath)
                bucket[match.absolutePath] = []
            }
            bucket[match.absolutePath]?.append(match)
        }
        return order.compactMap { path in
            guard let entries = bucket[path], let first = entries.first else { return nil }
            return FileMatchGroup(
                id: path,
                absolutePath: path,
                relativePath: first.relativePath,
                matches: entries
            )
        }
    }
}

private struct FileGroupHeader: View {
    let group: FileMatchGroup

    private var fileName: String {
        (group.relativePath as NSString).lastPathComponent
    }

    private var directory: String {
        (group.relativePath as NSString).deletingLastPathComponent
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Text(fileName)
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
            if !directory.isEmpty {
                Text(directory)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer(minLength: UIMetrics.spacing4)
            Text("\(group.matches.count)")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                .foregroundStyle(MuxyTheme.bg)
                .padding(.horizontal, UIMetrics.scaled(7))
                .padding(.vertical, UIMetrics.spacing1)
                .background(Capsule().fill(MuxyTheme.fgMuted))
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing3)
    }
}

private struct MatchRow: View {
    let match: TextSearchMatch
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        highlightedSnippet
            .font(.system(size: UIMetrics.fontBody, design: .monospaced))
            .lineLimit(1)
            .truncationMode(.tail)
            .padding(.leading, UIMetrics.spacing9)
            .padding(.trailing, UIMetrics.spacing6)
            .padding(.vertical, UIMetrics.scaled(3))
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
            .onHover { hovered = $0 }
    }

    private var highlightedSnippet: Text {
        let trimmed = trimLeadingWhitespace(match.lineText)
        let utf8 = Array(match.lineText.utf8)
        let removed = utf8.count - Array(trimmed.utf8).count

        let adjustedStart = max(0, match.matchStart - removed)
        let adjustedEnd = max(adjustedStart, match.matchEnd - removed)
        let trimmedUTF8 = Array(trimmed.utf8)

        guard match.matchStart >= 0,
              match.matchEnd <= utf8.count,
              match.matchStart < match.matchEnd,
              adjustedEnd <= trimmedUTF8.count
        else {
            return Text(trimmed).foregroundColor(MuxyTheme.fgDim)
        }

        let prefix = String(data: Data(trimmedUTF8[0 ..< adjustedStart]), encoding: .utf8) ?? ""
        let middle = String(data: Data(trimmedUTF8[adjustedStart ..< adjustedEnd]), encoding: .utf8) ?? ""
        let suffix = String(data: Data(trimmedUTF8[adjustedEnd ..< trimmedUTF8.count]), encoding: .utf8) ?? ""
        return Text(prefix).foregroundColor(MuxyTheme.fgDim)
            + Text(middle).foregroundColor(MuxyTheme.fg).bold()
            + Text(suffix).foregroundColor(MuxyTheme.fgDim)
    }

    private func trimLeadingWhitespace(_ text: String) -> String {
        var index = text.startIndex
        while index < text.endIndex, text[index] == " " || text[index] == "\t" {
            index = text.index(after: index)
        }
        return String(text[index...])
    }
}
