import Foundation

enum AutoIndentExtension {
    struct Edit: Equatable {
        var replaceRange: NSRange
        var replacement: String
        var selectionAfter: Int
    }

    static func edit(
        forReturnIn content: NSString,
        selection: NSRange,
        tabSize: Int,
        indentUnit: String
    ) -> Edit {
        let loc = min(selection.location, content.length)
        let selectionEnd = min(NSMaxRange(selection), content.length)
        let lineRange = content.lineRange(for: NSRange(location: loc, length: 0))
        let lineStart = lineRange.location
        let lineEnd = lineEndExcludingNewline(lineRange: lineRange, content: content)

        let prefixRange = NSRange(location: lineStart, length: loc - lineStart)
        let prefix = content.substring(with: prefixRange)
        let leading = leadingWhitespace(of: prefix)
        let prefixContent = prefix.dropFirst(leading.count)
        let prefixTrimmedTrailing = trimmingTrailingSpaces(prefix)
        let trailingWhitespaceCount = prefix.count - prefixTrimmedTrailing.count

        let suffixStart = min(selectionEnd, lineEnd)
        let suffixRange = NSRange(location: suffixStart, length: max(0, lineEnd - suffixStart))
        let suffix = suffixRange.length > 0 ? content.substring(with: suffixRange) : ""
        let suffixTrimmedLeadingCount = suffix.prefix { $0 == " " || $0 == "\t" }.count
        let suffixTrimmed = String(suffix.dropFirst(suffixTrimmedLeadingCount))

        let replaceLocation = loc - trailingWhitespaceCount
        let replaceLength = selectionEnd - replaceLocation

        if let marker = listOrCommentContinuation(leading: leading, prefixContent: String(prefixContent)) {
            if marker.cancelIfEmpty, prefixContent.trimmingCharacters(in: .whitespaces).isEmpty {
                let replacement = "\n"
                return Edit(
                    replaceRange: NSRange(location: lineStart, length: replaceLocation - lineStart + replaceLength),
                    replacement: replacement,
                    selectionAfter: lineStart + (replacement as NSString).length
                )
            }
            let continuation = marker.nextLinePrefix
            let replacement = "\n" + continuation
            return Edit(
                replaceRange: NSRange(location: replaceLocation, length: replaceLength),
                replacement: replacement,
                selectionAfter: replaceLocation + (replacement as NSString).length
            )
        }

        let opensBlock = endsWithOpenBracket(prefixTrimmedTrailing)
        let newIndent = opensBlock ? leading + indentUnit : leading

        if opensBlock,
           let suffixFirst = suffixTrimmed.first,
           isMatchingCloser(openLine: prefixTrimmedTrailing, suffixFirst: suffixFirst)
        {
            let closerChar = String(suffixFirst)
            let closerTail = String(suffixTrimmed.dropFirst())
            let middle = "\n" + newIndent
            let closingLine = "\n" + leading + closerChar + closerTail
            let replacement = middle + closingLine
            let replaceEnd = suffixStart + suffixTrimmedLeadingCount + suffixTrimmed.count
            return Edit(
                replaceRange: NSRange(location: replaceLocation, length: replaceEnd - replaceLocation),
                replacement: replacement,
                selectionAfter: replaceLocation + (middle as NSString).length
            )
        }

        let replacement = "\n" + newIndent
        return Edit(
            replaceRange: NSRange(location: replaceLocation, length: replaceLength),
            replacement: replacement,
            selectionAfter: replaceLocation + (replacement as NSString).length
        )
    }

    static func edit(
        forClosing char: Character,
        in content: NSString,
        selection: NSRange,
        indentUnit: String
    ) -> Edit? {
        guard selection.length == 0 else { return nil }
        let loc = min(selection.location, content.length)
        let lineRange = content.lineRange(for: NSRange(location: loc, length: 0))
        let lineStart = lineRange.location
        let prefixRange = NSRange(location: lineStart, length: loc - lineStart)
        let prefix = content.substring(with: prefixRange)
        guard !prefix.isEmpty, prefix.allSatisfy({ $0 == " " || $0 == "\t" }) else { return nil }
        let dedented = dedent(prefix, by: indentUnit)
        guard dedented.count < prefix.count else { return nil }
        let replacement = dedented + String(char)
        return Edit(
            replaceRange: NSRange(location: lineStart, length: loc - lineStart),
            replacement: replacement,
            selectionAfter: lineStart + (replacement as NSString).length
        )
    }

    static func detectIndentUnit(in content: NSString, tabSize: Int) -> String {
        let spacesFallback = String(repeating: " ", count: max(1, tabSize))
        let length = content.length
        guard length > 0 else { return spacesFallback }
        var index = 0
        var linesScanned = 0
        let maxLines = 200
        while index < length, linesScanned < maxLines {
            let lineRange = content.lineRange(for: NSRange(location: index, length: 0))
            let lineEnd = lineEndExcludingNewline(lineRange: lineRange, content: content)
            if lineEnd > lineRange.location {
                let first = content.character(at: lineRange.location)
                if first == 0x09 {
                    return "\t"
                }
                if first == 0x20 {
                    var spaceCount = 0
                    var scan = lineRange.location
                    while scan < lineEnd, content.character(at: scan) == 0x20 {
                        spaceCount += 1
                        scan += 1
                    }
                    if spaceCount > 0 {
                        return String(repeating: " ", count: min(spaceCount, max(1, tabSize)))
                    }
                }
            }
            let next = NSMaxRange(lineRange)
            if next <= index { break }
            index = next
            linesScanned += 1
        }
        return spacesFallback
    }

    static func leadingWhitespace(of line: String) -> String {
        String(line.prefix { $0 == " " || $0 == "\t" })
    }

    static func dedent(_ indent: String, by unit: String) -> String {
        guard !unit.isEmpty else { return indent }
        if unit == "\t" {
            if indent.hasSuffix("\t") {
                return String(indent.dropLast())
            }
            return indent
        }
        let spaceCount = unit.count
        var trimmed = indent
        var removed = 0
        while removed < spaceCount, trimmed.last == " " {
            trimmed.removeLast()
            removed += 1
        }
        if removed == 0, trimmed.last == "\t" {
            trimmed.removeLast()
        }
        return trimmed
    }

    private static func endsWithOpenBracket(_ text: String) -> Bool {
        guard let last = text.last else { return false }
        return last == "{" || last == "(" || last == "["
    }

    private static func isMatchingCloser(openLine: String, suffixFirst: Character) -> Bool {
        guard let opener = openLine.last else { return false }
        switch (opener, suffixFirst) {
        case ("{", "}"),
             ("(", ")"),
             ("[", "]"):
            return true
        default:
            return false
        }
    }

    private static func trimmingTrailingSpaces(_ text: String) -> String {
        var result = text
        while let last = result.last, last == " " || last == "\t" {
            result.removeLast()
        }
        return result
    }

    private static func lineEndExcludingNewline(lineRange: NSRange, content: NSString) -> Int {
        var end = NSMaxRange(lineRange)
        while end > lineRange.location {
            let ch = content.character(at: end - 1)
            if ch == 0x0A || ch == 0x0D {
                end -= 1
                continue
            }
            break
        }
        return end
    }

    struct MarkerContinuation {
        var nextLinePrefix: String
        var cancelIfEmpty: Bool
    }

    private static func listOrCommentContinuation(leading: String, prefixContent: String) -> MarkerContinuation? {
        if prefixContent.hasPrefix("//") {
            let afterSlashes = prefixContent.dropFirst(2)
            let spacer = afterSlashes.first == " " ? " " : ""
            return MarkerContinuation(nextLinePrefix: leading + "//" + spacer, cancelIfEmpty: false)
        }
        if prefixContent.hasPrefix("#"), !prefixContent.hasPrefix("#!") {
            let afterHash = prefixContent.dropFirst(1)
            let spacer = afterHash.first == " " ? " " : ""
            return MarkerContinuation(nextLinePrefix: leading + "#" + spacer, cancelIfEmpty: false)
        }
        if let bullet = bulletMarker(prefixContent) {
            return MarkerContinuation(nextLinePrefix: leading + bullet, cancelIfEmpty: true)
        }
        if let ordered = orderedListMarker(prefixContent) {
            return MarkerContinuation(nextLinePrefix: leading + ordered, cancelIfEmpty: true)
        }
        return nil
    }

    private static func bulletMarker(_ text: String) -> String? {
        guard let first = text.first, first == "-" || first == "*" || first == "+" else { return nil }
        let rest = text.dropFirst()
        guard rest.first == " " else { return nil }
        return "\(first) "
    }

    private static func orderedListMarker(_ text: String) -> String? {
        var digits = ""
        for ch in text {
            if ch.isNumber {
                digits.append(ch)
            } else {
                break
            }
        }
        guard !digits.isEmpty else { return nil }
        let afterDigits = text.dropFirst(digits.count)
        guard afterDigits.first == "." else { return nil }
        let afterDot = afterDigits.dropFirst()
        guard afterDot.first == " " else { return nil }
        return "\(digits). "
    }
}
