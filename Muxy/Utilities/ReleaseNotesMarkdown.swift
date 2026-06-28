import Foundation

struct ReleaseNotesSpan: Equatable {
    let text: String
    let isBold: Bool
}

enum ReleaseNotesBlock: Equatable {
    case heading(level: Int, spans: [ReleaseNotesSpan])
    case bullet(spans: [ReleaseNotesSpan])
    case paragraph(spans: [ReleaseNotesSpan])
}

enum ReleaseNotesMarkdown {
    static func parse(_ markdown: String) -> [ReleaseNotesBlock] {
        var blocks: [ReleaseNotesBlock] = []

        for rawLine in markdown.split(whereSeparator: \.isNewline) {
            let line = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !line.isEmpty else { continue }

            if let heading = parseHeading(line) {
                blocks.append(heading)
                continue
            }
            if let bullet = parseBullet(line) {
                blocks.append(bullet)
                continue
            }
            blocks.append(.paragraph(spans: parseInline(line)))
        }

        return blocks
    }

    private static func parseHeading(_ line: String) -> ReleaseNotesBlock? {
        var level = 0
        var remainder = Substring(line)
        while remainder.first == "#" {
            level += 1
            remainder = remainder.dropFirst()
        }
        guard level > 0, remainder.first == " " else { return nil }
        let content = remainder.trimmingCharacters(in: .whitespaces)
        guard !content.isEmpty else { return nil }
        return .heading(level: min(level, 6), spans: parseInline(content))
    }

    private static func parseBullet(_ line: String) -> ReleaseNotesBlock? {
        for marker in ["- ", "* ", "+ "] where line.hasPrefix(marker) {
            let content = String(line.dropFirst(marker.count)).trimmingCharacters(in: .whitespaces)
            guard !content.isEmpty else { return nil }
            return .bullet(spans: parseInline(content))
        }
        return nil
    }

    private static func parseInline(_ text: String) -> [ReleaseNotesSpan] {
        let stripped = stripLinks(text)
        var spans: [ReleaseNotesSpan] = []
        var buffer = ""
        var isBold = false
        var index = stripped.startIndex

        while index < stripped.endIndex {
            if stripped[index...].hasPrefix("**") {
                if !buffer.isEmpty {
                    spans.append(ReleaseNotesSpan(text: buffer, isBold: isBold))
                    buffer = ""
                }
                isBold.toggle()
                index = stripped.index(index, offsetBy: 2)
                continue
            }
            buffer.append(stripped[index])
            index = stripped.index(after: index)
        }

        if !buffer.isEmpty {
            spans.append(ReleaseNotesSpan(text: buffer, isBold: isBold))
        }
        return spans
    }

    private static func stripLinks(_ text: String) -> String {
        let pattern = #"\[([^\]]*)\]\([^)]*\)"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return text }
        let range = NSRange(text.startIndex..., in: text)
        return regex.stringByReplacingMatches(in: text, range: range, withTemplate: "$1")
    }
}
