import AppKit
import SwiftUI

let diffLineHeight: CGFloat = 20

enum DiffChunk {
    case divider(text: String)
    case codeBlock(rows: [DiffDisplayRow])
}

func buildDiffChunks(from rows: [DiffDisplayRow]) -> [DiffChunk] {
    var chunks: [DiffChunk] = []
    var currentRows: [DiffDisplayRow] = []

    for row in rows {
        if row.kind == .hunk || row.kind == .collapsed {
            if !currentRows.isEmpty {
                chunks.append(.codeBlock(rows: currentRows))
                currentRows = []
            }
            let label = row.kind == .hunk ? hunkLabel(row.text) : row.text
            chunks.append(.divider(text: label))
        } else {
            currentRows.append(row)
        }
    }

    if !currentRows.isEmpty {
        chunks.append(.codeBlock(rows: currentRows))
    }

    return chunks
}

func buildDiffMetadata(from rows: [DiffDisplayRow]) -> [DiffLineMetadata] {
    rows.map {
        DiffLineMetadata(
            kind: $0.kind,
            oldLineNumber: $0.oldLineNumber,
            newLineNumber: $0.newLineNumber
        )
    }
}

struct DiffRenderedBlock: @unchecked Sendable {
    let attributedString: NSAttributedString
    let metadata: [DiffLineMetadata]
}

struct DiffRenderedBundle: @unchecked Sendable {
    let block: DiffRenderedBlock
    let backgrounds: [NSColor?]
}

func buildDiffAttributedString(
    from rows: [DiffDisplayRow],
    theme: DiffRenderTheme
) -> DiffRenderedBlock {
    var metadata: [DiffLineMetadata] = []
    let result = NSMutableAttributedString()
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
        case .addition: theme.additionColor
        case .deletion: theme.deletionColor
        default: theme.defaultColor
        }

        let lineAttr = NSMutableAttributedString(
            string: lineText,
            attributes: [
                .foregroundColor: baseColor,
                .font: theme.font,
                .paragraphStyle: paragraphStyle,
            ]
        )

        let highlightRange = NSRange(location: 0, length: (lineText as NSString).length)
        for rule in theme.rules {
            let matches = rule.regex.matches(in: lineText, range: highlightRange)
            for match in matches {
                lineAttr.addAttribute(.foregroundColor, value: rule.color, range: match.range)
            }
        }

        if index > 0 {
            result.append(NSAttributedString(string: "\n", attributes: [
                .font: theme.font,
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

    return DiffRenderedBlock(attributedString: result, metadata: metadata)
}

struct DiffContentBridge: NSViewRepresentable {
    let rows: [DiffDisplayRow]
    let backgroundSide: DiffBackgroundSide

    final class Coordinator {
        var configuredSignature = Int.min
        var buildTask: Task<Void, Never>?

        deinit {
            buildTask?.cancel()
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context _: Context) -> DiffContentNSView {
        DiffContentNSView(frame: .zero)
    }

    func updateNSView(_ nsView: DiffContentNSView, context: Context) {
        let signature = contentSignature
        guard signature != context.coordinator.configuredSignature else { return }
        context.coordinator.configuredSignature = signature
        context.coordinator.buildTask?.cancel()

        let capturedRows = rows
        let side = backgroundSide
        let theme = DiffRenderTheme.current()

        let maxColumns = Self.maxDisplayColumns(in: capturedRows)
        nsView.prepareSize(
            rowCount: capturedRows.count,
            maxColumns: maxColumns,
            lineHeight: diffLineHeight
        )

        context.coordinator.buildTask = Task { [weak nsView] in
            let rendered = await GitProcessRunner.offMain {
                let block = buildDiffAttributedString(from: capturedRows, theme: theme)
                let backgrounds = buildLineBackgrounds(metadata: block.metadata, side: side, theme: theme)
                return DiffRenderedBundle(block: block, backgrounds: backgrounds)
            }
            guard !Task.isCancelled, let nsView else { return }
            nsView.configure(
                attributedString: rendered.block.attributedString,
                metadata: rendered.block.metadata,
                lineBackgrounds: rendered.backgrounds,
                lineHeight: diffLineHeight
            )
        }
    }

    private static func maxDisplayColumns(in rows: [DiffDisplayRow]) -> Int {
        var maxColumns = 0
        for row in rows {
            let text: String = switch row.kind {
            case .deletion: row.oldText ?? ""
            case .addition: row.newText ?? ""
            default: row.newText ?? row.oldText ?? ""
            }
            let count = text.utf16.count
            if count > maxColumns { maxColumns = count }
        }
        return maxColumns
    }

    private var contentSignature: Int {
        var hasher = Hasher()
        hasher.combine(backgroundSideHash(backgroundSide))
        hasher.combine(rows.count)
        for row in rows {
            hasher.combine(diffRowKindHash(row.kind))
            hasher.combine(row.oldLineNumber)
            hasher.combine(row.newLineNumber)
            hasher.combine(row.oldText)
            hasher.combine(row.newText)
            hasher.combine(row.text)
        }
        return hasher.finalize()
    }
}

struct DiffGutterBridge: NSViewRepresentable {
    let metadata: [DiffLineMetadata]
    let filePath: String
    let mode: DiffGutterMode
    let columnWidth: CGFloat

    final class Coordinator {
        var configuredSignature = Int.min
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
        let signature = gutterSignature
        guard signature != context.coordinator.configuredSignature else { return }
        configureView(nsView, context: context)
    }

    private var gutterSignature: Int {
        var hasher = Hasher()
        hasher.combine(filePath)
        hasher.combine(gutterModeHash(mode))
        hasher.combine(columnWidth)
        hasher.combine(metadata.count)
        for line in metadata {
            hasher.combine(diffRowKindHash(line.kind))
            hasher.combine(line.oldLineNumber)
            hasher.combine(line.newLineNumber)
        }
        return hasher.finalize()
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
        context.coordinator.configuredSignature = gutterSignature
    }
}

private func diffRowKindHash(_ kind: DiffDisplayRow.Kind) -> Int {
    switch kind {
    case .hunk: 1
    case .context: 2
    case .addition: 3
    case .deletion: 4
    case .collapsed: 5
    }
}

private func backgroundSideHash(_ side: DiffBackgroundSide) -> Int {
    switch side {
    case .left: 1
    case .right: 2
    case .both: 3
    }
}

private func gutterModeHash(_ mode: DiffGutterMode) -> Int {
    switch mode {
    case .unified: 1
    case .singleOld: 2
    case .singleNew: 3
    }
}
