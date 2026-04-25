import Foundation

enum MarkdownCodeBlockHighlighter {
    private static let fenceRegex = try? NSRegularExpression(
        pattern: "(^|\\n)([ \\t]{0,8})```([^\\n`]*)\\r?\\n([\\s\\S]*?)\\r?\\n[ \\t]{0,8}```(?=\\n|$)",
        options: []
    )

    static func prerenderCodeBlocks(in markdown: String) -> String {
        guard let fenceRegex else { return markdown }
        let nsMarkdown = markdown as NSString
        let fullRange = NSRange(location: 0, length: nsMarkdown.length)
        let matches = fenceRegex.matches(in: markdown, options: [], range: fullRange)
        guard !matches.isEmpty else { return markdown }

        let mutable = NSMutableString(string: markdown)
        for match in matches.reversed() {
            guard match.numberOfRanges >= 5 else { continue }
            let leadRange = match.range(at: 1)
            let infoRange = match.range(at: 3)
            let bodyRange = match.range(at: 4)
            guard infoRange.location != NSNotFound, bodyRange.location != NSNotFound else { continue }

            let info = nsMarkdown.substring(with: infoRange).trimmingCharacters(in: .whitespaces)
            let firstToken = info.split(whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? ""
            if firstToken.lowercased() == "mermaid" {
                continue
            }

            let body = nsMarkdown.substring(with: bodyRange)
            let highlighted = highlight(body: body, languageHint: firstToken)
            let replacement = renderBlock(html: highlighted, languageHint: firstToken)

            let lead = leadRange.location == NSNotFound ? "" : nsMarkdown.substring(with: leadRange)
            let prefix = lead.isEmpty ? "\n\n" : (lead == "\n" ? "\n\n" : lead + "\n")
            mutable.replaceCharacters(in: match.range, with: prefix + replacement + "\n")
        }
        return mutable as String
    }

    private static func highlight(body: String, languageHint: String) -> String {
        if !languageHint.isEmpty,
           let grammar = SyntaxLanguageRegistry.grammar(forLanguageHint: languageHint)
        {
            return SyntaxHTMLRenderer.render(source: body, grammar: grammar)
        }
        return SyntaxHTMLRenderer.escape(body)
    }

    private static func renderBlock(html: String, languageHint: String) -> String {
        let langClass = languageHint.isEmpty
            ? "muxy-hl"
            : "muxy-hl muxy-hl-lang-\(SyntaxHTMLRenderer.escape(languageHint.lowercased()))"
        return "<pre class=\"muxy-prehl\"><code class=\"\(langClass)\">\(html)</code></pre>"
    }
}
