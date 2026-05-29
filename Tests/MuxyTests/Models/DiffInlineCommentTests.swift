import CoreGraphics
import Testing

@testable import Muxy

@Suite("DiffCommentAnchor")
struct DiffCommentAnchorTests {
    private func makeRows() -> [DiffDisplayRow] {
        [
            DiffDisplayRow(kind: .context, oldLineNumber: 10, newLineNumber: 10, oldText: "a", newText: "a", text: "a"),
            DiffDisplayRow(kind: .deletion, oldLineNumber: 11, newLineNumber: nil, oldText: "old", newText: nil, text: "old"),
            DiffDisplayRow(kind: .addition, oldLineNumber: nil, newLineNumber: 11, oldText: nil, newText: "new", text: "new"),
            DiffDisplayRow(kind: .context, oldLineNumber: 12, newLineNumber: 12, oldText: "b", newText: "b", text: "b"),
        ]
    }

    @Test("resolves a new-side line to its display row index")
    func resolvesNewSide() {
        let rows = makeRows()
        #expect(DiffCommentAnchor.resolveDisplayRowIndex(side: .new, line: 11, in: rows) == 2)
        #expect(DiffCommentAnchor.resolveDisplayRowIndex(side: .new, line: 12, in: rows) == 3)
    }

    @Test("resolves an old-side line to its display row index")
    func resolvesOldSide() {
        let rows = makeRows()
        #expect(DiffCommentAnchor.resolveDisplayRowIndex(side: .old, line: 11, in: rows) == 1)
    }

    @Test("returns nil when the line no longer exists after a reload")
    func returnsNilForOutdatedLine() {
        let rows = makeRows()
        #expect(DiffCommentAnchor.resolveDisplayRowIndex(side: .new, line: 999, in: rows) == nil)
    }

    @Test("builds a snippet covering the commented line range")
    func buildsSnippet() {
        let rows = makeRows()
        let comment = DiffInlineComment(
            cacheKey: "file",
            filePath: "file.swift",
            side: .new,
            startLine: 11,
            endLine: 12,
            body: "fix",
            author: "me",
            createdAt: .distantPast
        )
        #expect(DiffCommentAnchor.snippet(for: comment, in: rows) == "new\nb")
    }
}

@Suite("DiffCommentLayout")
@MainActor
struct DiffCommentLayoutTests {
    private func rows() -> [DiffRenderedRow] {
        [
            DiffRenderedRow(oldLineNumber: 10, newLineNumber: 10),
            DiffRenderedRow(oldLineNumber: nil, newLineNumber: 11),
            DiffRenderedRow(oldLineNumber: 12, newLineNumber: 12),
        ]
    }

    private func comment(line: Int, side: DiffCommentSide = .new) -> DiffInlineComment {
        DiffInlineComment(
            cacheKey: "file",
            filePath: "file.swift",
            side: side,
            startLine: line,
            endLine: line,
            body: "short",
            author: "me",
            createdAt: .distantPast
        )
    }

    @Test("empty layout when there are no comments or composer")
    func emptyLayout() {
        let layout = DiffCommentLayout.make(renderedRows: rows(), comments: [], composer: nil, composerLineCount: 1)
        #expect(layout.blocks.isEmpty)
        #expect(layout.totalExtraHeight == 0)
    }

    @Test("one block anchored to the comment's rendered row")
    func anchorsBlockToRow() {
        let layout = DiffCommentLayout.make(renderedRows: rows(), comments: [comment(line: 11)], composer: nil, composerLineCount: 1)
        #expect(layout.blocks.count == 1)
        #expect(layout.blocks.first?.anchorRowIndex == 1)
        #expect(layout.blocks.first?.side == .new)
        #expect(layout.totalExtraHeight > 0)
    }

    @Test("each side reserves an equal gap at the same row")
    func bothSidesReserveSameGap() {
        let layout = DiffCommentLayout.make(renderedRows: rows(), comments: [comment(line: 11)], composer: nil, composerLineCount: 1)
        let block = layout.blocks.first
        #expect(block != nil)
        #expect(layout.gaps[1] == (block?.height ?? 0) + DiffCommentLayout.verticalInset * 2)
        #expect(layout.totalExtraHeight == layout.gaps.values.reduce(0, +))
    }

    @Test("a taller composer reserves more height")
    func composerGrowsWithLineCount() {
        let target = DiffCommentComposerTarget(cacheKey: "file", filePath: "file.swift", side: .new, startLine: 11, endLine: 11)
        let single = DiffCommentLayout.make(renderedRows: rows(), comments: [], composer: target, composerLineCount: 1)
        let multi = DiffCommentLayout.make(renderedRows: rows(), comments: [], composer: target, composerLineCount: 4)
        #expect(multi.totalExtraHeight > single.totalExtraHeight)
    }

    @Test("outdated comments are skipped in the layout")
    func skipsOutdatedComments() {
        var outdated = comment(line: 11)
        outdated.submissionState = .outdated
        let layout = DiffCommentLayout.make(renderedRows: rows(), comments: [outdated], composer: nil, composerLineCount: 1)
        #expect(layout.blocks.isEmpty)
        #expect(layout.totalExtraHeight == 0)
    }
}
