import SwiftUI

struct ExtensionModalOverlay: View {
    let request: ExtensionModalService.Request
    let onSelect: (ExtensionModalService.Item) -> Void
    let onDismiss: () -> Void

    var body: some View {
        PaletteOverlay<ExtensionModalService.Item>(
            placeholder: request.placeholder,
            emptyLabel: request.emptyLabel,
            noMatchLabel: request.noMatchLabel,
            pageSize: ExtensionModalService.pageSize,
            loadPage: { query, offset, limit in
                let page = await (try? ExtensionModalService.shared.loadPage(
                    for: request,
                    query: query,
                    offset: offset,
                    limit: limit
                )) ?? ExtensionModalService.Page(items: [], hasMore: false)
                return PaletteOverlay.Page(items: page.items, hasMore: page.hasMore)
            },
            onSelect: onSelect,
            onDismiss: onDismiss,
            row: { item, isHighlighted in
                AnyView(ExtensionModalRow(item: item, isHighlighted: isHighlighted))
            }
        )
        .onDisappear {
            ExtensionModalService.shared.dismiss(requestID: request.id)
        }
    }
}

private struct ExtensionModalRow: View {
    let item: ExtensionModalService.Item
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        HStack(spacing: UIMetrics.spacing4) {
            VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
                Text(item.title)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                if let subtitle = item.subtitle {
                    Text(subtitle)
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
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
