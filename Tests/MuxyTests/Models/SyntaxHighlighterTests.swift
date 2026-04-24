import Foundation
import Testing

@testable import Muxy

@Suite("SyntaxHighlighter")
@MainActor
struct SyntaxHighlighterTests {
    private func store(_ text: String) -> TextBackingStore {
        let s = TextBackingStore()
        s.loadFromText(text)
        return s
    }

    @Test("tokens populated after full edit")
    func fullEditPopulatesCache() {
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let s = store("let x = 1\nlet y = 2\n")
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: s.lineCount, backingStore: s)
        #expect(highlighter.tokens(forLine: 0) != nil)
        #expect(highlighter.tokens(forLine: 1) != nil)
    }

    @Test("reset clears cache")
    func resetClears() {
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let s = store("let x = 1")
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: 1, backingStore: s)
        #expect(highlighter.tokens(forLine: 0) != nil)
        highlighter.reset()
        #expect(highlighter.tokens(forLine: 0) == nil)
    }

    @Test("invalidate removes tokens from line onward")
    func invalidateFromLine() {
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let s = store("a\nb\nc\n")
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: s.lineCount, backingStore: s)
        highlighter.invalidate(fromLine: 1)
        #expect(highlighter.tokens(forLine: 0) != nil)
        #expect(highlighter.tokens(forLine: 1) == nil)
        #expect(highlighter.tokens(forLine: 2) == nil)
    }

    @Test("cascade outcome when opening an unclosed block comment")
    func cascadeOnBlockComment() {
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let s = store("let x = 1\nlet y = 2\nlet z = 3\n")
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: s.lineCount, backingStore: s)

        _ = s.replaceLines(in: 0 ..< 1, with: ["/* open"])
        let outcome = highlighter.applyEdit(
            startLine: 0,
            oldLineCount: 1,
            newLineCount: 1,
            backingStore: s
        )
        #expect(outcome == .cascade)
    }

    @Test("no cascade for simple in-line edit")
    func noCascadeForSimpleEdit() {
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let s = store("let x = 1\nlet y = 2\n")
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: s.lineCount, backingStore: s)

        _ = s.replaceLines(in: 0 ..< 1, with: ["let x = 42"])
        let outcome = highlighter.applyEdit(
            startLine: 0,
            oldLineCount: 1,
            newLineCount: 1,
            backingStore: s
        )
        #expect(outcome == .updated)
    }

    @Test("spans returns tokens offset by line start")
    func spansOffsetByLineStart() {
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let s = store("let a = 1\nlet b = 2\n")
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: s.lineCount, backingStore: s)

        let offsets = [0, 10]
        let spans = highlighter.spans(in: 0 ..< 2, lineStartOffsets: offsets, backingStore: s)
        #expect(!spans.isEmpty)
        let line1Spans = spans.filter { $0.range.location >= 10 }
        #expect(!line1Spans.isEmpty)
    }

    @Test("closing */ across lines returns cascade then stabilizes")
    func closingBlockCommentStabilizes() {
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let s = store("/* open\nstill\nend */\nlet x = 1\n")
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: s.lineCount, backingStore: s)

        let line3 = highlighter.tokens(forLine: 3)
        #expect(line3 != nil)
        #expect(line3?.contains(where: { $0.scope == .keyword }) == true)
    }
}
