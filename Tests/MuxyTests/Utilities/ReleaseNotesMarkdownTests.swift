import Testing

@testable import Muxy

@Suite("ReleaseNotesMarkdown")
struct ReleaseNotesMarkdownTests {
    @Test("parses headings with their level")
    func headings() {
        let blocks = ReleaseNotesMarkdown.parse("# Title\n## Section\n### Sub")
        #expect(blocks == [
            .heading(level: 1, spans: [ReleaseNotesSpan(text: "Title", isBold: false)]),
            .heading(level: 2, spans: [ReleaseNotesSpan(text: "Section", isBold: false)]),
            .heading(level: 3, spans: [ReleaseNotesSpan(text: "Sub", isBold: false)]),
        ])
    }

    @Test("parses bullet points across markers")
    func bullets() {
        let blocks = ReleaseNotesMarkdown.parse("- one\n* two\n+ three")
        #expect(blocks == [
            .bullet(spans: [ReleaseNotesSpan(text: "one", isBold: false)]),
            .bullet(spans: [ReleaseNotesSpan(text: "two", isBold: false)]),
            .bullet(spans: [ReleaseNotesSpan(text: "three", isBold: false)]),
        ])
    }

    @Test("parses bold spans inline")
    func bold() {
        let blocks = ReleaseNotesMarkdown.parse("Added **fast** mode")
        #expect(blocks == [
            .paragraph(spans: [
                ReleaseNotesSpan(text: "Added ", isBold: false),
                ReleaseNotesSpan(text: "fast", isBold: true),
                ReleaseNotesSpan(text: " mode", isBold: false),
            ]),
        ])
    }

    @Test("strips links to plain text")
    func links() {
        let blocks = ReleaseNotesMarkdown.parse("- See [the docs](https://muxy.app/docs) now")
        #expect(blocks == [
            .bullet(spans: [ReleaseNotesSpan(text: "See the docs now", isBold: false)]),
        ])
    }

    @Test("ignores blank lines")
    func blankLines() {
        let blocks = ReleaseNotesMarkdown.parse("# Title\n\n\n- item")
        #expect(blocks == [
            .heading(level: 1, spans: [ReleaseNotesSpan(text: "Title", isBold: false)]),
            .bullet(spans: [ReleaseNotesSpan(text: "item", isBold: false)]),
        ])
    }

    @Test("treats a hash without a space as a paragraph")
    func hashWithoutSpace() {
        let blocks = ReleaseNotesMarkdown.parse("#notaheading")
        #expect(blocks == [
            .paragraph(spans: [ReleaseNotesSpan(text: "#notaheading", isBold: false)]),
        ])
    }

    @Test("handles carriage-return line endings")
    func crlfLineEndings() {
        let blocks = ReleaseNotesMarkdown.parse("# Title\r\n- item\r\n")
        #expect(blocks == [
            .heading(level: 1, spans: [ReleaseNotesSpan(text: "Title", isBold: false)]),
            .bullet(spans: [ReleaseNotesSpan(text: "item", isBold: false)]),
        ])
    }

    @Test("empty input yields no blocks")
    func empty() {
        #expect(ReleaseNotesMarkdown.parse("").isEmpty)
        #expect(ReleaseNotesMarkdown.parse("\n\n").isEmpty)
    }
}
