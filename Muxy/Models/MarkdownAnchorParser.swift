import Foundation

enum MarkdownAnchorParser {
    static func parseAnchors(in markdown: String) -> [MarkdownSyncAnchor] {
        let lines = splitLines(markdown)
        guard !lines.isEmpty else { return [] }

        var anchors: [MarkdownSyncAnchor] = []
        var index = 0

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)

            if trimmed.isEmpty {
                index += 1
                continue
            }

            if let fence = fenceStart(in: trimmed) {
                let endIndex = findFenceEnd(for: fence, lines: lines, startIndex: index)
                anchors.append(makeAnchor(kind: fence.kind, startIndex: index, endIndex: endIndex, ordinal: anchors.count))
                index = endIndex + 1
                continue
            }

            if isHeading(trimmed) {
                anchors.append(makeAnchor(kind: .heading, startIndex: index, endIndex: index, ordinal: anchors.count))
                index += 1
                continue
            }

            if isThematicBreak(trimmed) {
                anchors.append(makeAnchor(kind: .thematicBreak, startIndex: index, endIndex: index, ordinal: anchors.count))
                index += 1
                continue
            }

            if isTableHeader(lines: lines, index: index) {
                let endIndex = consumeTable(lines: lines, startIndex: index)
                anchors.append(makeAnchor(kind: .table, startIndex: index, endIndex: endIndex, ordinal: anchors.count))
                index = endIndex + 1
                continue
            }

            if isStandaloneImage(trimmed) {
                anchors.append(makeAnchor(kind: .image, startIndex: index, endIndex: index, ordinal: anchors.count))
                index += 1
                continue
            }

            if isHTMLBlockStart(trimmed) {
                let endIndex = consumeHTMLBlock(lines: lines, startIndex: index)
                anchors.append(makeAnchor(kind: .htmlBlock, startIndex: index, endIndex: endIndex, ordinal: anchors.count))
                index = endIndex + 1
                continue
            }

            if isBlockquote(trimmed) {
                let endIndex = consumeBlockquote(lines: lines, startIndex: index)
                anchors.append(makeAnchor(kind: .blockquote, startIndex: index, endIndex: endIndex, ordinal: anchors.count))
                index = endIndex + 1
                continue
            }

            if isListStart(trimmed) {
                let endIndex = consumeList(lines: lines, startIndex: index)
                anchors.append(makeAnchor(kind: .list, startIndex: index, endIndex: endIndex, ordinal: anchors.count))
                index = endIndex + 1
                continue
            }

            let endIndex = consumeParagraph(lines: lines, startIndex: index)
            anchors.append(makeAnchor(kind: .paragraph, startIndex: index, endIndex: endIndex, ordinal: anchors.count))
            index = endIndex + 1
        }

        return anchors
    }

    private struct FenceStart {
        let marker: Character
        let count: Int
        let kind: MarkdownSyncAnchorKind
    }

    private static func splitLines(_ markdown: String) -> [String] {
        markdown
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\r", with: "\n")
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)
    }

    private static func makeAnchor(kind: MarkdownSyncAnchorKind, startIndex: Int, endIndex: Int, ordinal: Int) -> MarkdownSyncAnchor {
        MarkdownSyncAnchor(
            id: "anchor-\(kind.rawValue)-\(ordinal + 1)",
            kind: kind,
            startLine: startIndex + 1,
            endLine: endIndex + 1
        )
    }

    private static func fenceStart(in trimmed: String) -> FenceStart? {
        guard let marker = trimmed.first, marker == "`" || marker == "~" else { return nil }
        let markerCount = trimmed.prefix { $0 == marker }.count
        guard markerCount >= 3 else { return nil }
        let rest = trimmed.dropFirst(markerCount).trimmingCharacters(in: .whitespaces)
        let info = rest.split(separator: " ", maxSplits: 1, omittingEmptySubsequences: true).first.map(String.init)?.lowercased()
        let kind: MarkdownSyncAnchorKind = info == "mermaid" ? .mermaid : .fencedCode
        return FenceStart(marker: marker, count: markerCount, kind: kind)
    }

    private static func findFenceEnd(for fence: FenceStart, lines: [String], startIndex: Int) -> Int {
        guard startIndex + 1 < lines.count else { return startIndex }
        for candidate in (startIndex + 1) ..< lines.count {
            let trimmed = lines[candidate].trimmingCharacters(in: .whitespaces)
            let prefixCount = trimmed.prefix { $0 == fence.marker }.count
            if prefixCount >= fence.count, trimmed.dropFirst(prefixCount).trimmingCharacters(in: .whitespaces).isEmpty {
                return candidate
            }
        }
        return lines.count - 1
    }

    private static func isHeading(_ trimmed: String) -> Bool {
        let hashes = trimmed.prefix { $0 == "#" }.count
        guard (1 ... 6).contains(hashes) else { return false }
        guard trimmed.count > hashes else { return false }
        let next = trimmed[trimmed.index(trimmed.startIndex, offsetBy: hashes)]
        return next.isWhitespace
    }

    private static func isThematicBreak(_ trimmed: String) -> Bool {
        let compact = trimmed.replacingOccurrences(of: " ", with: "")
        guard compact.count >= 3 else { return false }
        guard let first = compact.first, first == "-" || first == "*" || first == "_" else { return false }
        return compact.allSatisfy { $0 == first }
    }

    private static func isTableHeader(lines: [String], index: Int) -> Bool {
        guard index + 1 < lines.count else { return false }
        let header = lines[index].trimmingCharacters(in: .whitespaces)
        let separator = lines[index + 1].trimmingCharacters(in: .whitespaces)
        guard header.contains("|") else { return false }
        return isTableSeparator(separator)
    }

    private static func isTableSeparator(_ trimmed: String) -> Bool {
        let compact = trimmed.replacingOccurrences(of: " ", with: "")
        guard compact.contains("|") else { return false }
        let allowed = CharacterSet(charactersIn: "|-:")
        return compact.unicodeScalars.allSatisfy { allowed.contains($0) } && compact.contains("-")
    }

    private static func consumeTable(lines: [String], startIndex: Int) -> Int {
        var index = startIndex + 2
        var last = startIndex + 1
        while index < lines.count {
            let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty || !trimmed.contains("|") {
                break
            }
            last = index
            index += 1
        }
        return last
    }

    private static func isStandaloneImage(_ trimmed: String) -> Bool {
        guard trimmed.hasPrefix("!["), trimmed.contains("]("), trimmed.hasSuffix(")") else { return false }
        return !trimmed.contains(" ") || trimmed.first == "!"
    }

    private static func isHTMLBlockStart(_ trimmed: String) -> Bool {
        guard trimmed.hasPrefix("<"), !trimmed.hasPrefix("<!--") else { return false }
        return !trimmed.hasPrefix("<http")
    }

    private static func consumeHTMLBlock(lines: [String], startIndex: Int) -> Int {
        var index = startIndex
        var last = startIndex
        while index < lines.count {
            let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
            if index > startIndex, trimmed.isEmpty {
                break
            }
            last = index
            index += 1
        }
        return last
    }

    private static func isBlockquote(_ trimmed: String) -> Bool {
        trimmed.hasPrefix(">")
    }

    private static func consumeBlockquote(lines: [String], startIndex: Int) -> Int {
        var index = startIndex
        var last = startIndex
        while index < lines.count {
            let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty {
                last = index
                index += 1
                continue
            }
            guard isBlockquote(trimmed) else { break }
            last = index
            index += 1
        }
        return last
    }

    private static func isListStart(_ trimmed: String) -> Bool {
        unorderedListMarkerLength(in: trimmed) != nil || orderedListMarkerLength(in: trimmed) != nil
    }

    private static func unorderedListMarkerLength(in trimmed: String) -> Int? {
        guard let first = trimmed.first, first == "-" || first == "*" || first == "+" else { return nil }
        guard trimmed.count > 1 else { return nil }
        let next = trimmed[trimmed.index(after: trimmed.startIndex)]
        return next.isWhitespace ? 1 : nil
    }

    private static func orderedListMarkerLength(in trimmed: String) -> Int? {
        var digits = 0
        for character in trimmed {
            if character.isNumber {
                digits += 1
                continue
            }
            guard digits > 0, character == "." || character == ")" else { return nil }
            let markerIndex = trimmed.index(trimmed.startIndex, offsetBy: digits + 1)
            guard markerIndex < trimmed.endIndex else { return nil }
            return trimmed[markerIndex].isWhitespace ? digits + 1 : nil
        }
        return nil
    }

    private static func consumeList(lines: [String], startIndex: Int) -> Int {
        var index = startIndex + 1
        var last = startIndex
        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty {
                break
            }
            if isListStart(trimmed) || isIndented(line) {
                last = index
                index += 1
                continue
            }
            break
        }
        return last
    }

    private static func isIndented(_ line: String) -> Bool {
        let count = line.prefix { $0 == " " || $0 == "\t" }.count
        return count >= 2
    }

    private static func consumeParagraph(lines: [String], startIndex: Int) -> Int {
        var index = startIndex + 1
        var last = startIndex
        while index < lines.count {
            let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty || startsNewBlock(lines: lines, index: index) {
                break
            }
            last = index
            index += 1
        }
        return last
    }

    private static func startsNewBlock(lines: [String], index: Int) -> Bool {
        guard index < lines.count else { return false }
        let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty {
            return false
        }
        if fenceStart(in: trimmed) != nil {
            return true
        }
        return isHeading(trimmed)
            || isThematicBreak(trimmed)
            || isTableHeader(lines: lines, index: index)
            || isStandaloneImage(trimmed)
            || isHTMLBlockStart(trimmed)
            || isBlockquote(trimmed)
            || isListStart(trimmed)
    }
}
