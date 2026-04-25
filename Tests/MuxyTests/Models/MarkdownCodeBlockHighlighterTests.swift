import Foundation
import Testing

@testable import Muxy

@Suite("MarkdownCodeBlockHighlighter")
struct MarkdownCodeBlockHighlighterTests {
    @Test("replaces fenced swift block with prehighlighted html")
    func replacesSwiftFence() {
        let markdown = """
        # Hello

        ```swift
        let x = 1
        ```
        """
        let result = MarkdownCodeBlockHighlighter.prerenderCodeBlocks(in: markdown)
        #expect(result.contains("<pre class=\"muxy-prehl\">"))
        #expect(result.contains("muxy-hl-lang-swift"))
        #expect(result.contains("muxy-tok-keyword"))
        #expect(!result.contains("```swift"))
    }

    @Test("leaves mermaid fence intact")
    func skipsMermaid() {
        let markdown = """
        ```mermaid
        graph TD; A-->B;
        ```
        """
        let result = MarkdownCodeBlockHighlighter.prerenderCodeBlocks(in: markdown)
        #expect(result.contains("```mermaid"))
        #expect(!result.contains("muxy-prehl"))
    }

    @Test("escapes content of unknown language as plain code")
    func unknownLanguageEscapesContent() {
        let markdown = """
        ```brainfuck
        <script>alert(1)</script>
        ```
        """
        let result = MarkdownCodeBlockHighlighter.prerenderCodeBlocks(in: markdown)
        #expect(result.contains("&lt;script&gt;"))
        #expect(!result.contains("<script>alert"))
        #expect(result.contains("<pre class=\"muxy-prehl\">"))
    }

    @Test("handles fence without language label")
    func noLanguage() {
        let markdown = """
        ```
        plain text & < >
        ```
        """
        let result = MarkdownCodeBlockHighlighter.prerenderCodeBlocks(in: markdown)
        #expect(result.contains("plain text &amp; &lt; &gt;"))
    }

    @Test("does not touch markdown without fences")
    func noFences() {
        let markdown = "# Title\n\nSome **bold** text."
        let result = MarkdownCodeBlockHighlighter.prerenderCodeBlocks(in: markdown)
        #expect(result == markdown)
    }
}
