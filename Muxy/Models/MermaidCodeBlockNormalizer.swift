import Foundation

enum MermaidCodeBlockNormalizer {
    private static let mermaidFenceRegex = try? NSRegularExpression(
        pattern: "```mermaid\\s*\\r?\\n([\\s\\S]*?)```",
        options: []
    )

    static func normalizeMermaidCodeBlocks(in markdown: String) -> String {
        guard let mermaidFenceRegex else { return markdown }

        let nsMarkdown = markdown as NSString
        let range = NSRange(location: 0, length: nsMarkdown.length)
        let matches = mermaidFenceRegex.matches(in: markdown, options: [], range: range)
        guard !matches.isEmpty else { return markdown }

        let mutable = NSMutableString(string: markdown)
        for match in matches.reversed() {
            guard match.numberOfRanges >= 2 else { continue }
            let codeRange = match.range(at: 1)
            guard codeRange.location != NSNotFound else { continue }

            let diagram = nsMarkdown.substring(with: codeRange)
            let normalized = normalizeLabelNewlines(in: diagram)
            if normalized != diagram {
                mutable.replaceCharacters(in: codeRange, with: normalized)
            }
        }

        return mutable as String
    }

    static func normalizeLabelNewlines(in diagram: String) -> String {
        var output = String()
        output.reserveCapacity(diagram.count)

        var bracketDepth = 0
        var cursor = diagram.startIndex

        while cursor < diagram.endIndex {
            let ch = diagram[cursor]

            if ch == "[" {
                bracketDepth += 1
                output.append(ch)
                cursor = diagram.index(after: cursor)
                continue
            }

            if ch == "]" {
                bracketDepth = max(0, bracketDepth - 1)
                output.append(ch)
                cursor = diagram.index(after: cursor)
                continue
            }

            let next = diagram.index(after: cursor)
            if bracketDepth > 0,
               ch == "\\",
               next < diagram.endIndex,
               diagram[next] == "n"
            {
                output.append("<br/>")
                cursor = diagram.index(after: next)
                continue
            }

            if bracketDepth > 0,
               ch.isNewline
            {
                output.append("<br/>")
                if String(ch) == "\r",
                   next < diagram.endIndex,
                   diagram[next] == "\n"
                {
                    cursor = diagram.index(after: next)
                } else {
                    cursor = next
                }
                continue
            }

            output.append(ch)
            cursor = next
        }

        return output
    }
}
