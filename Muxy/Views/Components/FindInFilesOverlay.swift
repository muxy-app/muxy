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
    @State private var options = TextSearchOptions()
    @State private var coordinator = SearchCoordinator()

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
                onArrowDown: { moveHighlight(1) },
                onPageUp: { moveHighlight(-PaletteSearchField.pageJump) },
                onPageDown: { moveHighlight(PaletteSearchField.pageJump) }
            )
            SearchOptionToggle(
                label: "Aa",
                isOn: options.caseSensitive,
                tooltip: "Match Case"
            ) {
                options.caseSensitive.toggle()
                performSearch(debounce: false)
            }
            SearchOptionToggle(
                label: "ab|",
                isOn: options.wholeWord,
                tooltip: "Match Whole Word"
            ) {
                options.wholeWord.toggle()
                performSearch(debounce: false)
            }
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
                try? await Task.sleep(for: .milliseconds(120))
                guard !Task.isCancelled else { return }
            }

            let found = await TextSearchService.search(
                query: currentQuery,
                in: projectPath,
                options: options,
                coordinator: coordinator
            )
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

    private static let maxSnippetCharacters = 200
    private static let leadingContextCharacters = 24

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: UIMetrics.spacing4) {
            Text("\(match.lineNumber)")
                .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(minWidth: UIMetrics.scaled(36), alignment: .trailing)
            snippet
                .font(.system(size: UIMetrics.fontBody, design: .monospaced))
                .lineLimit(1)
                .truncationMode(.tail)
        }
        .padding(.leading, UIMetrics.spacing6)
        .padding(.trailing, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.scaled(3))
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .onHover { hovered = $0 }
    }

    private var snippet: Text {
        let line = match.lineText
        let utf8 = line.utf8
        let total = utf8.count

        guard match.matchByteStart >= 0,
              match.matchByteLength > 0,
              match.matchByteStart + match.matchByteLength <= total,
              let startScalar = utf8Index(line, byteOffset: match.matchByteStart),
              let endScalar = utf8Index(line, byteOffset: match.matchByteStart + match.matchByteLength)
        else {
            return Text(trimmed(line)).foregroundColor(MuxyTheme.fgDim)
        }

        let beforeMatch = String(line[..<startScalar])
        let middle = String(line[startScalar ..< endScalar])
        let afterMatch = String(line[endScalar...])

        let leadingTrimmed = trimLeadingWhitespace(beforeMatch)
        let (prefixDisplay, prefixEllipsis) = truncatedPrefix(leadingTrimmed)
        let (suffixDisplay, suffixEllipsis) = truncatedSuffix(afterMatch)

        let prefixPart = Text(prefixEllipsis ? "…" : "").foregroundColor(MuxyTheme.fgDim)
            + Text(prefixDisplay).foregroundColor(MuxyTheme.fgDim)
        let matchPart = Text(middle).foregroundColor(MuxyTheme.fg).bold()
        let suffixPart = Text(suffixDisplay).foregroundColor(MuxyTheme.fgDim)
            + Text(suffixEllipsis ? "…" : "").foregroundColor(MuxyTheme.fgDim)
        return prefixPart + matchPart + suffixPart
    }

    private func utf8Index(_ string: String, byteOffset: Int) -> String.Index? {
        let utf8 = string.utf8
        guard byteOffset >= 0, byteOffset <= utf8.count else { return nil }
        let index = utf8.index(utf8.startIndex, offsetBy: byteOffset)
        return index.samePosition(in: string)
    }

    private func trimmed(_ text: String) -> String {
        trimLeadingWhitespace(text)
    }

    private func trimLeadingWhitespace(_ text: String) -> String {
        var index = text.startIndex
        while index < text.endIndex, text[index] == " " || text[index] == "\t" {
            index = text.index(after: index)
        }
        return String(text[index...])
    }

    private func truncatedPrefix(_ text: String) -> (String, Bool) {
        let limit = Self.leadingContextCharacters
        guard text.count > limit else { return (text, false) }
        let start = text.index(text.endIndex, offsetBy: -limit)
        return (String(text[start...]), true)
    }

    private func truncatedSuffix(_ text: String) -> (String, Bool) {
        let limit = Self.maxSnippetCharacters - Self.leadingContextCharacters
        guard text.count > limit else { return (text, false) }
        let end = text.index(text.startIndex, offsetBy: limit)
        return (String(text[..<end]), true)
    }
}

private struct SearchOptionToggle: View {
    let label: String
    let isOn: Bool
    let tooltip: String
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(label)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                .foregroundStyle(isOn ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(28), height: UIMetrics.scaled(22))
                .background(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .fill(isOn ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .stroke(isOn ? MuxyTheme.border : .clear, lineWidth: 1)
                )
        }
        .buttonStyle(.plain)
        .help(tooltip)
        .onHover { hovered = $0 }
    }
}
