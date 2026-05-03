import AppKit

struct MarkdownInlineDecoration: Equatable {
    enum Kind: Equatable {
        case heading(level: Int)
        case bold
        case italic
        case boldItalic
        case strikethrough
        case codeSpan
        case marker
        case blockquote
        case listMarker
    }

    let range: NSRange
    let kind: Kind
}

enum MarkdownInlineHighlighter {
    static func decorations(line: String, isInsideFencedCode: Bool) -> [MarkdownInlineDecoration] {
        guard !isInsideFencedCode else { return [] }
        let ns = line as NSString
        let length = ns.length
        guard length > 0 else { return [] }

        var decorations: [MarkdownInlineDecoration] = []
        let leading = leadingWhitespaceCount(ns: ns, length: length)
        var contentStart = leading

        if let blockquote = matchBlockquote(ns: ns, length: length, from: leading) {
            decorations.append(blockquote.decoration)
            contentStart = blockquote.contentStart
        } else if let list = matchListMarker(ns: ns, length: length, from: leading) {
            decorations.append(list.decoration)
            contentStart = list.contentStart
        }

        if let heading = matchHeading(ns: ns, length: length, from: contentStart) {
            decorations.append(contentsOf: heading)
            return decorations
        }

        decorations.append(contentsOf: scanInline(ns: ns, length: length, from: contentStart))
        return decorations
    }

    private static func leadingWhitespaceCount(ns: NSString, length: Int) -> Int {
        var index = 0
        while index < length {
            let ch = ns.character(at: index)
            if ch == 0x20 || ch == 0x09 { index += 1 } else { break }
        }
        if index > 3 { return 0 }
        return index
    }

    private static func matchBlockquote(
        ns: NSString,
        length: Int,
        from start: Int
    ) -> (decoration: MarkdownInlineDecoration, contentStart: Int)? {
        guard start < length, ns.character(at: start) == 0x3E else { return nil }
        var end = start + 1
        if end < length, ns.character(at: end) == 0x20 { end += 1 }
        let decoration = MarkdownInlineDecoration(
            range: NSRange(location: start, length: end - start),
            kind: .blockquote
        )
        return (decoration, end)
    }

    private static func matchListMarker(
        ns: NSString,
        length: Int,
        from start: Int
    ) -> (decoration: MarkdownInlineDecoration, contentStart: Int)? {
        guard start < length else { return nil }
        let ch = ns.character(at: start)
        if ch == 0x2D || ch == 0x2A || ch == 0x2B {
            let next = start + 1
            guard next < length, ns.character(at: next) == 0x20 else { return nil }
            return (
                MarkdownInlineDecoration(range: NSRange(location: start, length: 1), kind: .listMarker),
                next + 1
            )
        }
        if ch >= 0x30, ch <= 0x39 {
            var index = start
            while index < length {
                let c = ns.character(at: index)
                if c >= 0x30, c <= 0x39 { index += 1 } else { break }
            }
            guard index < length else { return nil }
            let punct = ns.character(at: index)
            guard punct == 0x2E || punct == 0x29 else { return nil }
            let after = index + 1
            guard after < length, ns.character(at: after) == 0x20 else { return nil }
            return (
                MarkdownInlineDecoration(
                    range: NSRange(location: start, length: after - start),
                    kind: .listMarker
                ),
                after + 1
            )
        }
        return nil
    }

    private static func matchHeading(
        ns: NSString,
        length: Int,
        from start: Int
    ) -> [MarkdownInlineDecoration]? {
        guard start < length, ns.character(at: start) == 0x23 else { return nil }
        var hashEnd = start
        while hashEnd < length, ns.character(at: hashEnd) == 0x23 {
            hashEnd += 1
        }
        let level = hashEnd - start
        guard level >= 1, level <= 6 else { return nil }
        guard hashEnd == length || ns.character(at: hashEnd) == 0x20 else { return nil }
        let headingRange = NSRange(location: start, length: length - start)
        let markerRange = NSRange(location: start, length: level)
        return [
            MarkdownInlineDecoration(range: headingRange, kind: .heading(level: level)),
            MarkdownInlineDecoration(range: markerRange, kind: .marker),
        ]
    }

    private static func scanInline(ns: NSString, length: Int, from start: Int) -> [MarkdownInlineDecoration] {
        var decorations: [MarkdownInlineDecoration] = []
        var index = start
        while index < length {
            let ch = ns.character(at: index)
            if ch == 0x60 {
                if let endIndex = findClosing(ns: ns, length: length, from: index + 1, char: 0x60) {
                    decorations.append(MarkdownInlineDecoration(
                        range: NSRange(location: index, length: endIndex - index + 1),
                        kind: .codeSpan
                    ))
                    index = endIndex + 1
                    continue
                }
            } else if ch == 0x2A || ch == 0x5F {
                if let match = matchEmphasis(ns: ns, length: length, from: index, marker: ch) {
                    decorations.append(match.decoration)
                    index = match.endIndex
                    continue
                }
            } else if ch == 0x7E {
                if let match = matchStrikethrough(ns: ns, length: length, from: index) {
                    decorations.append(match.decoration)
                    index = match.endIndex
                    continue
                }
            }
            index += 1
        }
        return decorations
    }

    private static func findClosing(ns: NSString, length: Int, from start: Int, char: unichar) -> Int? {
        var index = start
        while index < length {
            if ns.character(at: index) == char { return index }
            index += 1
        }
        return nil
    }

    private static func matchEmphasis(
        ns: NSString,
        length: Int,
        from start: Int,
        marker: unichar
    ) -> (decoration: MarkdownInlineDecoration, endIndex: Int)? {
        var openCount = 0
        var cursor = start
        while cursor < length, ns.character(at: cursor) == marker, openCount < 3 {
            openCount += 1
            cursor += 1
        }
        guard openCount >= 1, cursor < length else { return nil }
        let firstContent = ns.character(at: cursor)
        if firstContent == 0x20 || firstContent == 0x09 { return nil }

        var search = cursor
        while search < length {
            if ns.character(at: search) == marker {
                var closeCount = 0
                var probe = search
                while probe < length, ns.character(at: probe) == marker, closeCount < openCount {
                    closeCount += 1
                    probe += 1
                }
                if closeCount == openCount, search > cursor {
                    let prev = ns.character(at: search - 1)
                    if prev != 0x20, prev != 0x09 {
                        let kind: MarkdownInlineDecoration.Kind = switch openCount {
                        case 1: .italic
                        case 2: .bold
                        default: .boldItalic
                        }
                        let totalLength = (probe - start)
                        return (
                            MarkdownInlineDecoration(
                                range: NSRange(location: start, length: totalLength),
                                kind: kind
                            ),
                            probe
                        )
                    }
                }
                search = probe
            } else {
                search += 1
            }
        }
        return nil
    }

    private static func matchStrikethrough(
        ns: NSString,
        length: Int,
        from start: Int
    ) -> (decoration: MarkdownInlineDecoration, endIndex: Int)? {
        guard start + 1 < length, ns.character(at: start + 1) == 0x7E else { return nil }
        var index = start + 2
        while index + 1 < length {
            if ns.character(at: index) == 0x7E, ns.character(at: index + 1) == 0x7E {
                let end = index + 2
                return (
                    MarkdownInlineDecoration(
                        range: NSRange(location: start, length: end - start),
                        kind: .strikethrough
                    ),
                    end
                )
            }
            index += 1
        }
        return nil
    }
}

@MainActor
enum MarkdownInlineStyle {
    static func font(for kind: MarkdownInlineDecoration.Kind, baseFont: NSFont) -> NSFont? {
        switch kind {
        case .heading:
            applyTraits(.bold, to: baseFont)
        case .bold:
            applyTraits(.bold, to: baseFont)
        case .italic:
            applyTraits(.italic, to: baseFont)
        case .boldItalic:
            applyTraits([.bold, .italic], to: baseFont)
        case .strikethrough,
             .codeSpan,
             .marker,
             .blockquote,
             .listMarker:
            nil
        }
    }

    static func foregroundColor(for kind: MarkdownInlineDecoration.Kind) -> NSColor? {
        switch kind {
        case .heading:
            SyntaxTheme.color(for: .heading)
        case .codeSpan:
            SyntaxTheme.color(for: .string)
        case .marker,
             .blockquote,
             .listMarker:
            SyntaxTheme.defaultForeground.withAlphaComponent(0.5)
        case .bold,
             .italic,
             .boldItalic,
             .strikethrough:
            nil
        }
    }

    static func strikethroughStyle(for kind: MarkdownInlineDecoration.Kind) -> NSUnderlineStyle? {
        kind == .strikethrough ? .single : nil
    }

    private static func applyTraits(_ traits: NSFontDescriptor.SymbolicTraits, to font: NSFont) -> NSFont {
        let descriptor = font.fontDescriptor.withSymbolicTraits(traits)
        return NSFont(descriptor: descriptor, size: font.pointSize) ?? font
    }
}
