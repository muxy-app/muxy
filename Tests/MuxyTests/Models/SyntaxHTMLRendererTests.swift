import Foundation
import Testing

@testable import Muxy

@Suite("SyntaxHTMLRenderer")
struct SyntaxHTMLRendererTests {
    @Test("escapes HTML special characters in plain text")
    func escapesPlainText() {
        let escaped = SyntaxHTMLRenderer.escape("a < b && c > d \"e\" 'f'")
        #expect(escaped == "a &lt; b &amp;&amp; c &gt; d &quot;e&quot; &#39;f&#39;")
    }

    @Test("wraps tokens in span with scope class")
    func wrapsTokensInSpans() {
        guard let grammar = SyntaxLanguageRegistry.grammar(forLanguageHint: "swift") else {
            Issue.record("missing swift grammar")
            return
        }
        let html = SyntaxHTMLRenderer.render(source: "let x = 1", grammar: grammar)
        #expect(html.contains("<span class=\"muxy-tok-keyword\">let</span>"))
        #expect(html.contains("<span class=\"muxy-tok-number\">1</span>"))
    }

    @Test("escapes angle brackets inside tokens")
    func escapesInsideTokens() {
        guard let grammar = SyntaxLanguageRegistry.grammar(forLanguageHint: "swift") else {
            Issue.record("missing swift grammar")
            return
        }
        let html = SyntaxHTMLRenderer.render(source: "let s = \"<script>\"", grammar: grammar)
        #expect(!html.contains("<script>"))
        #expect(html.contains("&lt;script&gt;"))
    }

    @Test("preserves line breaks across multi-line input")
    func preservesNewlines() {
        guard let grammar = SyntaxLanguageRegistry.grammar(forLanguageHint: "swift") else {
            Issue.record("missing swift grammar")
            return
        }
        let html = SyntaxHTMLRenderer.render(source: "let a = 1\nlet b = 2", grammar: grammar)
        #expect(html.contains("\n"))
    }
}
