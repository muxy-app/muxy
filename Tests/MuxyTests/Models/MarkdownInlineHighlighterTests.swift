import Foundation
import Testing

@testable import Muxy

@Suite("MarkdownInlineHighlighter")
struct MarkdownInlineHighlighterTests {
    @Test("ATX heading produces heading + marker decoration")
    func heading() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "## Hello world", isInsideFencedCode: false)
        #expect(decorations.contains { $0.kind == .heading(level: 2) })
        #expect(decorations.contains { $0.kind == .marker && $0.range.length == 2 })
    }

    @Test("Six-level headings detected; seven hashes are not headings")
    func headingLevels() {
        for level in 1 ... 6 {
            let line = String(repeating: "#", count: level) + " Title"
            let decorations = MarkdownInlineHighlighter.decorations(line: line, isInsideFencedCode: false)
            #expect(decorations.contains { $0.kind == .heading(level: level) })
        }
        let invalid = MarkdownInlineHighlighter.decorations(line: "####### nope", isInsideFencedCode: false)
        #expect(!invalid.contains { if case .heading = $0.kind { true } else { false } })
    }

    @Test("Bold detected with ** markers")
    func bold() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "this is **strong** text", isInsideFencedCode: false)
        let bold = decorations.first { $0.kind == .bold }
        #expect(bold != nil)
        #expect(bold?.range.length == "**strong**".count)
    }

    @Test("Italic detected with single asterisk markers")
    func italic() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "an *emphasized* word", isInsideFencedCode: false)
        #expect(decorations.contains { $0.kind == .italic })
    }

    @Test("Bold italic detected with triple asterisks")
    func boldItalic() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "***wow***", isInsideFencedCode: false)
        #expect(decorations.contains { $0.kind == .boldItalic })
    }

    @Test("Strikethrough detected")
    func strikethrough() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "~~old~~ news", isInsideFencedCode: false)
        #expect(decorations.contains { $0.kind == .strikethrough })
    }

    @Test("Code span detected")
    func codeSpan() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "use `foo()` here", isInsideFencedCode: false)
        #expect(decorations.contains { $0.kind == .codeSpan })
    }

    @Test("Blockquote marker detected")
    func blockquote() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "> a quote", isInsideFencedCode: false)
        #expect(decorations.contains { $0.kind == .blockquote })
    }

    @Test("Unordered and ordered list markers detected")
    func listMarkers() {
        let dash = MarkdownInlineHighlighter.decorations(line: "- item", isInsideFencedCode: false)
        #expect(dash.contains { $0.kind == .listMarker })

        let numbered = MarkdownInlineHighlighter.decorations(line: "1. item", isInsideFencedCode: false)
        #expect(numbered.contains { $0.kind == .listMarker })
    }

    @Test("No decorations inside fenced code block")
    func fencedCode() {
        let decorations = MarkdownInlineHighlighter.decorations(
            line: "# not a heading **inside** code",
            isInsideFencedCode: true
        )
        #expect(decorations.isEmpty)
    }

    @Test("Unmatched markers do not produce emphasis")
    func unmatched() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "this **never closes", isInsideFencedCode: false)
        #expect(!decorations.contains { $0.kind == .bold })
    }

    @Test("Whitespace-flanked markers are not emphasis")
    func whitespaceFlanked() {
        let decorations = MarkdownInlineHighlighter.decorations(line: "a ** b ** c", isInsideFencedCode: false)
        #expect(!decorations.contains { $0.kind == .bold })
    }
}
