import SwiftUI

struct QuickOpenOverlay: View {
    let projectPath: String
    let onSelect: (String) -> Void
    let onDismiss: () -> Void

    var body: some View {
        PaletteOverlay<FileSearchResult>(
            placeholder: "Search files by name...",
            emptyLabel: "No files found",
            noMatchLabel: "No matching files",
            search: { query in
                await FileSearchService.search(query: query, in: projectPath)
            },
            onSelect: { result in onSelect(result.absolutePath) },
            onDismiss: onDismiss,
            row: { result, isHighlighted in
                AnyView(FileResultRow(result: result, isHighlighted: isHighlighted))
            }
        )
    }
}

private struct FileResultRow: View {
    let result: FileSearchResult
    let isHighlighted: Bool
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
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: fileIcon)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconLG)
            VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
                Text(result.fileName)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                Text(result.relativePath)
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer()
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing3)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .onHover { isHovered in
            hovered = isHovered
        }
    }
}
