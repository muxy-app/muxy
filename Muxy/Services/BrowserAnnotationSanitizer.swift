import Foundation

enum BrowserAnnotationSanitizer {
    static let maxSelectorLength = 512
    static let maxXPathLength = 1024
    static let maxTextSnippetLength = 1024
    static let maxURLLength = 2048
    static let maxTitleLength = 512
    static let maxCommentLength = 4096
    static let maxStyleValueLength = 256
    static let maxOuterHTMLLength = 1024
    static let maxStylesheetCount = 3
    static let maxStylesheetURLLength = 512

    static func sanitizeSingleLine(_ value: String, maxLength: Int) -> String {
        let stripped = stripControlCharacters(value, allowNewlines: false, allowTabs: false)
        return clip(stripped, to: maxLength)
    }

    static func sanitizeMultiLine(_ value: String, maxLength: Int) -> String {
        let stripped = stripControlCharacters(value, allowNewlines: true, allowTabs: true)
        return clip(stripped, to: maxLength)
    }

    static func sanitizeMarkdownInlineCode(_ value: String, maxLength: Int) -> String {
        let single = sanitizeSingleLine(value, maxLength: maxLength)
        return single.replacingOccurrences(of: "`", with: "ʼ")
    }

    static func sanitizeURLString(_ value: String) -> String {
        sanitizeSingleLine(value, maxLength: maxURLLength)
    }

    static func sanitizeStyleValue(_ value: String) -> String {
        let single = sanitizeSingleLine(value, maxLength: maxStyleValueLength)
        return single
            .replacingOccurrences(of: ";", with: "")
            .replacingOccurrences(of: "{", with: "")
            .replacingOccurrences(of: "}", with: "")
            .replacingOccurrences(of: "<", with: "")
            .replacingOccurrences(of: ">", with: "")
    }

    static func sanitizeOuterHTML(_ value: String) -> String {
        let stripped = stripControlCharacters(value, allowNewlines: true, allowTabs: true)
        let collapsed = collapseWhitespace(stripped)
        let withoutFence = collapsed.replacingOccurrences(of: "```", with: "ʼʼʼ")
        return clip(withoutFence, to: maxOuterHTMLLength)
    }

    static func sanitizeDirection(_ value: String) -> String {
        let normalized = sanitizeSingleLine(value, maxLength: 16).lowercased()
        switch normalized {
        case "ltr",
             "rtl",
             "auto": return normalized
        default: return ""
        }
    }

    static func sanitizeLanguageCode(_ value: String) -> String {
        let trimmed = sanitizeSingleLine(value, maxLength: 35)
        guard !trimmed.isEmpty else { return "" }
        let pattern = "^[A-Za-z]{1,8}(-[A-Za-z0-9]{1,8})*$"
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return "" }
        let range = NSRange(trimmed.startIndex ..< trimmed.endIndex, in: trimmed)
        guard regex.firstMatch(in: trimmed, range: range) != nil else { return "" }
        return trimmed
    }

    static func sanitizeStylesheetList(_ values: [String]) -> [String] {
        var seen: Set<String> = []
        var result: [String] = []
        for raw in values {
            let cleaned = sanitizeSingleLine(raw, maxLength: maxStylesheetURLLength)
            guard !cleaned.isEmpty else { continue }
            guard !seen.contains(cleaned) else { continue }
            seen.insert(cleaned)
            result.append(cleaned)
            if result.count >= maxStylesheetCount { break }
        }
        return result
    }

    private static func collapseWhitespace(_ value: String) -> String {
        var result = String.UnicodeScalarView()
        result.reserveCapacity(value.unicodeScalars.count)
        var lastWasSpace = false
        for scalar in value.unicodeScalars {
            let isSpace = scalar == " " || scalar == "\t"
            if isSpace {
                if lastWasSpace { continue }
                lastWasSpace = true
                result.append(" ")
            } else {
                lastWasSpace = false
                result.append(scalar)
            }
        }
        return String(result)
    }

    private static func stripControlCharacters(_ value: String, allowNewlines: Bool, allowTabs: Bool) -> String {
        var result = String.UnicodeScalarView()
        result.reserveCapacity(value.unicodeScalars.count)
        for scalar in value.unicodeScalars {
            let codepoint = scalar.value
            if allowNewlines, codepoint == 0x0A { result.append(scalar)
                continue
            }
            if allowTabs, codepoint == 0x09 { result.append(scalar)
                continue
            }
            if codepoint < 0x20 { continue }
            if codepoint == 0x7F { continue }
            if codepoint >= 0x80, codepoint <= 0x9F { continue }
            result.append(scalar)
        }
        return String(result)
    }

    private static func clip(_ value: String, to maxLength: Int) -> String {
        guard value.count > maxLength else { return value }
        let index = value.index(value.startIndex, offsetBy: maxLength)
        return String(value[..<index])
    }
}
