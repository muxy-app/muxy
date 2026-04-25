import Foundation
import Testing

@testable import Muxy

@Suite("MarkdownAnchorParser")
struct MarkdownAnchorParserTests {
    @Test("parses headings and paragraphs with inclusive line ranges")
    func parsesHeadingsAndParagraphs() {
        let markdown = """
        # Title

        First paragraph line 1
        First paragraph line 2

        ## Subtitle
        Next paragraph
        """

        let anchors = MarkdownAnchorParser.parseAnchors(in: markdown)

        #expect(anchors.map(\.kind) == [.heading, .paragraph, .heading, .paragraph])
        #expect(anchors.map(\.startLine) == [1, 3, 6, 7])
        #expect(anchors.map(\.endLine) == [1, 4, 6, 7])
    }

    @Test("parses unordered and ordered lists as list anchors")
    func parsesLists() {
        let markdown = """
        - First
          continuation
        - Second

        1. One
        2. Two
        """

        let anchors = MarkdownAnchorParser.parseAnchors(in: markdown)

        #expect(anchors.count == 2)
        #expect(anchors[0].kind == .list)
        #expect(anchors[0].startLine == 1)
        #expect(anchors[0].endLine == 3)
        #expect(anchors[1].kind == .list)
        #expect(anchors[1].startLine == 5)
        #expect(anchors[1].endLine == 6)
    }

    @Test("parses fenced code and mermaid blocks distinctly")
    func parsesFencedCodeAndMermaid() {
        let markdown = """
        ```swift
        let x = 1
        ```

        ```mermaid
        graph TD
        A --> B
        ```
        """

        let anchors = MarkdownAnchorParser.parseAnchors(in: markdown)

        #expect(anchors.count == 2)
        #expect(anchors[0].kind == .fencedCode)
        #expect(anchors[0].startLine == 1)
        #expect(anchors[0].endLine == 3)
        #expect(anchors[1].kind == .mermaid)
        #expect(anchors[1].startLine == 5)
        #expect(anchors[1].endLine == 8)
    }

    @Test("parses tables, standalone images, and thematic breaks")
    func parsesTablesImagesAndBreaks() {
        let markdown = """
        | Col A | Col B |
        | --- | --- |
        | 1 | 2 |

        ![Diagram](diagram.png)

        ---
        """

        let anchors = MarkdownAnchorParser.parseAnchors(in: markdown)

        #expect(anchors.map(\.kind) == [.table, .image, .thematicBreak])
        #expect(anchors.map(\.startLine) == [1, 5, 7])
        #expect(anchors.map(\.endLine) == [3, 5, 7])
    }

    @Test("parses blockquotes and html blocks")
    func parsesBlockquotesAndHTMLBlocks() {
        let markdown = """
        > Quote line 1
        > Quote line 2

        <div>
        raw html
        </div>
        """

        let anchors = MarkdownAnchorParser.parseAnchors(in: markdown)

        #expect(anchors.count == 2)
        #expect(anchors[0].kind == .blockquote)
        #expect(anchors[0].startLine == 1)
        #expect(anchors[0].endLine == 3)
        #expect(anchors[1].kind == .htmlBlock)
        #expect(anchors[1].startLine == 4)
        #expect(anchors[1].endLine == 6)
    }

    @Test("skips blank lines and produces stable ids")
    func stableIDs() {
        let markdown = """

        # Title

        Paragraph
        """

        let anchors = MarkdownAnchorParser.parseAnchors(in: markdown)

        #expect(anchors.count == 2)
        #expect(anchors[0].id == "anchor-heading-1")
        #expect(anchors[1].id == "anchor-paragraph-2")
    }

    @Test("anchor ids stay stable when leading lines are inserted")
    func stableIDsAcrossEdits() {
        let before = """
        # Title

        Paragraph
        """

        let after = """


        # Title

        Paragraph
        """

        let beforeAnchors = MarkdownAnchorParser.parseAnchors(in: before)
        let afterAnchors = MarkdownAnchorParser.parseAnchors(in: after)

        #expect(beforeAnchors.map(\.id) == afterAnchors.map(\.id))
    }
}
