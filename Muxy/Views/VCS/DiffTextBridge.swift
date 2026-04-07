import AppKit
import SwiftUI

private let diffLineHeight: CGFloat = 20
@MainActor private let diffFont = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)

enum DiffChunk: Identifiable {
    case divider(id: UUID, text: String, isHunk: Bool)
    case codeBlock(id: UUID, rows: [DiffDisplayRow])

    var id: UUID {
        switch self {
        case let .divider(id, _, _): id
        case let .codeBlock(id, _): id
        }
    }
}

func buildDiffChunks(from rows: [DiffDisplayRow]) -> [DiffChunk] {
    var chunks: [DiffChunk] = []
    var currentRows: [DiffDisplayRow] = []

    for row in rows {
        if row.kind == .hunk || row.kind == .collapsed {
            if !currentRows.isEmpty {
                chunks.append(.codeBlock(id: UUID(), rows: currentRows))
                currentRows = []
            }
            let label = row.kind == .hunk ? hunkLabel(row.text) : row.text
            chunks.append(.divider(id: UUID(), text: label, isHunk: row.kind == .hunk))
        } else {
            currentRows.append(row)
        }
    }

    if !currentRows.isEmpty {
        chunks.append(.codeBlock(id: UUID(), rows: currentRows))
    }

    return chunks
}

@MainActor
func buildDiffAttributedString(
    from rows: [DiffDisplayRow]
) -> (NSAttributedString, [DiffLineMetadata]) {
    var metadata: [DiffLineMetadata] = []
    let result = NSMutableAttributedString()
    let rules = DiffHighlightCache.shared.rules
    let paragraphStyle = NSMutableParagraphStyle()
    paragraphStyle.minimumLineHeight = diffLineHeight
    paragraphStyle.maximumLineHeight = diffLineHeight

    for (index, row) in rows.enumerated() {
        let lineText: String
        let kind: DiffDisplayRow.Kind

        switch row.kind {
        case .deletion:
            lineText = row.oldText ?? ""
            kind = .deletion
        case .addition:
            lineText = row.newText ?? ""
            kind = .addition
        default:
            lineText = row.newText ?? row.oldText ?? ""
            kind = row.kind
        }

        let baseColor: NSColor = switch kind {
        case .addition: MuxyTheme.nsDiffAdd
        case .deletion: MuxyTheme.nsDiffRemove
        default: GhosttyService.shared.foregroundColor
        }

        let lineAttr = NSMutableAttributedString(
            string: lineText,
            attributes: [
                .foregroundColor: baseColor,
                .font: diffFont,
                .paragraphStyle: paragraphStyle,
            ]
        )

        let highlightRange = NSRange(location: 0, length: (lineText as NSString).length)
        for rule in rules {
            let matches = rule.regex.matches(in: lineText, range: highlightRange)
            let color = rule.color()
            for match in matches {
                lineAttr.addAttribute(.foregroundColor, value: color, range: match.range)
            }
        }

        if index > 0 {
            result.append(NSAttributedString(string: "\n", attributes: [
                .font: diffFont,
                .paragraphStyle: paragraphStyle,
            ]))
        }
        result.append(lineAttr)

        metadata.append(DiffLineMetadata(
            kind: row.kind,
            oldLineNumber: row.oldLineNumber,
            newLineNumber: row.newLineNumber
        ))
    }

    return (result, metadata)
}

struct DiffContentBridge: NSViewRepresentable {
    let rows: [DiffDisplayRow]
    let backgroundSide: DiffBackgroundSide

    final class Coordinator {
        var configuredRowCount = -1
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> DiffContentNSView {
        let view = DiffContentNSView(frame: .zero)
        configureView(view, context: context)
        return view
    }

    func updateNSView(_ nsView: DiffContentNSView, context: Context) {
        guard rows.count != context.coordinator.configuredRowCount else { return }
        configureView(nsView, context: context)
    }

    private func configureView(_ view: DiffContentNSView, context: Context) {
        let (attrString, metadata) = buildDiffAttributedString(from: rows)
        let backgrounds = buildLineBackgrounds(metadata: metadata, side: backgroundSide)
        view.configure(
            attributedString: attrString,
            metadata: metadata,
            lineBackgrounds: backgrounds,
            lineHeight: diffLineHeight
        )
        context.coordinator.configuredRowCount = rows.count
    }
}

struct DiffGutterBridge: NSViewRepresentable {
    let metadata: [DiffLineMetadata]
    let filePath: String
    let mode: DiffGutterMode
    let columnWidth: CGFloat

    final class Coordinator {
        var configuredCount = -1
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> DiffGutterNSView {
        let view = DiffGutterNSView(frame: .zero)
        configureView(view, context: context)
        return view
    }

    func updateNSView(_ nsView: DiffGutterNSView, context: Context) {
        guard metadata.count != context.coordinator.configuredCount else { return }
        configureView(nsView, context: context)
    }

    private func configureView(_ view: DiffGutterNSView, context: Context) {
        view.lineMetadata = metadata
        view.filePath = filePath
        view.mode = mode
        view.columnWidth = columnWidth
        view.lineHeight = diffLineHeight
        view.cachedBorderColor = MuxyTheme.nsBg.blended(
            withFraction: 0.12,
            of: GhosttyService.shared.foregroundColor
        ) ?? .separatorColor
        view.cachedNumberColor = GhosttyService.shared.foregroundColor.withAlphaComponent(0.4)
        view.cachedNumberHoverColor = GhosttyService.shared.foregroundColor.withAlphaComponent(0.85)
        view.cachedAddColor = MuxyTheme.nsDiffAdd
        view.cachedRemoveColor = MuxyTheme.nsDiffRemove
        view.invalidateIntrinsicContentSize()
        view.needsDisplay = true
        context.coordinator.configuredCount = metadata.count
    }
}
