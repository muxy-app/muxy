import SwiftUI

struct BrowserSuggestionList: View {
    let model: BrowserSuggestionModel
    let onSelect: (BrowserHistoryEntry) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(model.suggestions.enumerated()), id: \.element.id) { index, entry in
                row(for: entry, isSelected: isSelected(index: index, entry: entry))
                    .contentShape(Rectangle())
                    .onTapGesture { onSelect(entry) }
                    .onHover { hovering in
                        if hovering {
                            model.hover(entry)
                        } else if model.hoveredEntryID == entry.id {
                            model.hover(nil)
                        }
                    }
            }
        }
        .padding(.vertical, UIMetrics.spacing1)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.bg)
        .background(MuxyTheme.surface)
        .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .strokeBorder(MuxyTheme.border, lineWidth: 1)
        )
    }

    private func isSelected(index: Int, entry: BrowserHistoryEntry) -> Bool {
        if index == model.selectedIndex {
            return true
        }
        guard model.selectedIndex == nil else {
            return false
        }
        if let hoveredEntryID = model.hoveredEntryID {
            return hoveredEntryID == entry.id
        }
        return false
    }

    private func row(for entry: BrowserHistoryEntry, isSelected: Bool) -> some View {
        HStack(spacing: UIMetrics.spacing3) {
            suggestionIcon(for: entry)
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)

            VStack(alignment: .leading, spacing: 0) {
                if let title = entry.title, !title.isEmpty {
                    Text(title)
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                }
                Text(entry.url)
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.spacing2)
        .background(isSelected ? MuxyTheme.hover : Color.clear)
    }

    @ViewBuilder
    private func suggestionIcon(for entry: BrowserHistoryEntry) -> some View {
        if let url = URL(string: entry.url), let favicon = FaviconStore.shared.favicon(for: url) {
            Image(nsImage: favicon)
                .resizable()
                .interpolation(.high)
        } else {
            Image(systemName: "clock.arrow.circlepath")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgDim)
        }
    }
}
