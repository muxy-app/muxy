import SwiftUI

struct FindInFilesOverlay: View {
    let projectPath: String
    let onSelect: (TextSearchMatch) -> Void
    let onDismiss: () -> Void

    var body: some View {
        PaletteOverlay<TextSearchMatch>(
            placeholder: "Search text in files...",
            emptyLabel: "Type at least \(TextSearchService.minQueryLength) characters",
            noMatchLabel: "No matches found",
            search: { query in
                await TextSearchService.search(query: query, in: projectPath)
            },
            onSelect: { match in onSelect(match) },
            onDismiss: onDismiss,
            row: { match, isHighlighted in
                AnyView(TextMatchRow(match: match, isHighlighted: isHighlighted))
            }
        )
    }
}

private struct TextMatchRow: View {
    let match: TextSearchMatch
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 1) {
            HStack(spacing: 6) {
                Image(systemName: "doc.text")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: 14)
                Text(match.relativePath)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Text(":\(match.lineNumber)")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                Spacer(minLength: 0)
            }
            highlightedSnippet
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.leading, 20)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .onHover { hovered = $0 }
    }

    private var highlightedSnippet: Text {
        let utf8 = Array(match.lineText.utf8)
        guard match.matchStart >= 0,
              match.matchEnd <= utf8.count,
              match.matchStart < match.matchEnd
        else {
            return Text(match.lineText)
        }
        let prefixData = Data(utf8[0 ..< match.matchStart])
        let matchData = Data(utf8[match.matchStart ..< match.matchEnd])
        let suffixData = Data(utf8[match.matchEnd ..< utf8.count])
        let prefix = String(data: prefixData, encoding: .utf8) ?? ""
        let middle = String(data: matchData, encoding: .utf8) ?? ""
        let suffix = String(data: suffixData, encoding: .utf8) ?? ""
        return Text(prefix)
            + Text(middle).foregroundColor(MuxyTheme.fg).bold()
            + Text(suffix)
    }
}
