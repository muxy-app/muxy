import CoreGraphics
import Foundation

struct DiffCommentBlock: Identifiable, Equatable {
    enum Content: Equatable {
        case composer(DiffCommentComposerTarget)
        case comment(DiffInlineComment)
    }

    let id: String
    let side: DiffCommentSide
    let anchorRowIndex: Int
    let height: CGFloat
    let content: Content
}

@MainActor
struct DiffCommentLayout: Equatable {
    let blocks: [DiffCommentBlock]
    let gaps: [Int: CGFloat]
    let totalExtraHeight: CGFloat

    static let empty = DiffCommentLayout(blocks: [], gaps: [:], totalExtraHeight: 0)

    private static let blockVerticalInset: CGFloat = 6
    private static let blockVerticalPadding: CGFloat = 8
    private static let bodyLineHeight: CGFloat = 17
    private static let authorRowHeight: CGFloat = 18
    private static let estimatedCharsPerLine: CGFloat = 72

    func blocks(side: DiffCommentSide, atRow row: Int) -> [DiffCommentBlock] {
        blocks.filter { $0.side == side && $0.anchorRowIndex == row }
    }

    static func make(
        renderedRows: [DiffRenderedRow],
        comments: [DiffInlineComment],
        composer: DiffCommentComposerTarget?,
        composerLineCount: Int
    ) -> DiffCommentLayout {
        var blocks: [DiffCommentBlock] = []

        func append(content: DiffCommentBlock.Content, side: DiffCommentSide, startLine: Int, endLine: Int) {
            let start = renderedRowIndex(side: side, line: startLine, in: renderedRows)
            let end = renderedRowIndex(side: side, line: endLine, in: renderedRows)
            guard let anchorRow = [start, end].compactMap(\.self).max() else { return }
            blocks.append(DiffCommentBlock(
                id: identifier(for: content),
                side: side,
                anchorRowIndex: anchorRow,
                height: height(for: content, composerLineCount: composerLineCount),
                content: content
            ))
        }

        for comment in comments where comment.submissionState != .outdated {
            append(content: .comment(comment), side: comment.side, startLine: comment.startLine, endLine: comment.endLine)
        }
        if let composer {
            append(content: .composer(composer), side: composer.side, startLine: composer.startLine, endLine: composer.endLine)
        }

        let gaps = gapHeights(for: blocks)
        let totalExtraHeight = gaps.values.reduce(0, +)
        return DiffCommentLayout(blocks: blocks, gaps: gaps, totalExtraHeight: totalExtraHeight)
    }

    private static func gapHeights(for blocks: [DiffCommentBlock]) -> [Int: CGFloat] {
        var heightsBySideAndRow: [DiffCommentSide: [Int: CGFloat]] = [.old: [:], .new: [:]]
        for block in blocks {
            let blockHeight = block.height + blockVerticalInset * 2
            heightsBySideAndRow[block.side, default: [:]][block.anchorRowIndex, default: 0] += blockHeight
        }
        var gaps: [Int: CGFloat] = [:]
        for side in [DiffCommentSide.old, .new] {
            for (row, height) in heightsBySideAndRow[side] ?? [:] {
                gaps[row] = max(gaps[row] ?? 0, height)
            }
        }
        return gaps
    }

    static var verticalInset: CGFloat { blockVerticalInset }

    static var composerMinFieldHeight: CGFloat { bodyLineHeight * 2 }

    private static func renderedRowIndex(side: DiffCommentSide, line: Int, in rows: [DiffRenderedRow]) -> Int? {
        rows.firstIndex { row in
            switch side {
            case .old: row.oldLineNumber == line
            case .new: row.newLineNumber == line
            }
        }
    }

    private static func identifier(for content: DiffCommentBlock.Content) -> String {
        switch content {
        case let .composer(target):
            "composer:\(target.side):\(target.startLine):\(target.endLine)"
        case let .comment(comment):
            "comment:\(comment.id.uuidString)"
        }
    }

    private static func height(for content: DiffCommentBlock.Content, composerLineCount: Int) -> CGFloat {
        switch content {
        case .composer:
            let fieldText = max(composerMinFieldHeight, CGFloat(max(1, composerLineCount)) * bodyLineHeight)
            let fieldBox = fieldText + UIMetrics.spacing3 * 2
            let buttonRow = UIMetrics.controlMedium + UIMetrics.spacing3 * 2
            return UIMetrics.spacing3 * 2 + fieldBox + 1 + buttonRow
        case let .comment(comment):
            return authorRowHeight + bodyHeight(comment.body) + blockVerticalPadding * 2
        }
    }

    private static func bodyHeight(_ body: String) -> CGFloat {
        let lines = body.split(separator: "\n", omittingEmptySubsequences: false)
        let wrappedLineCount = lines.reduce(0) { partial, line in
            partial + max(1, Int(ceil(CGFloat(line.count) / estimatedCharsPerLine)))
        }
        return CGFloat(max(1, wrappedLineCount)) * bodyLineHeight
    }
}
