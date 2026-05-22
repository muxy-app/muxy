import Foundation
import Testing

@testable import Muxy

@Suite("BrowserAnnotationSanitizer")
struct BrowserAnnotationSanitizerTests {
    @Test("strips C0 control characters except newline and tab in multiline mode")
    func stripsC0Multiline() {
        let dangerous = "hello\u{1B}[2Jworld\u{07}\nline\twith\u{00}null"
        let sanitized = BrowserAnnotationSanitizer.sanitizeMultiLine(dangerous, maxLength: 1024)
        #expect(sanitized == "hello[2Jworld\nline\twithnull")
    }

    @Test("strips all control characters in single-line mode")
    func stripsControlsSingleLine() {
        let dangerous = "hello\nworld\twith\u{1B}[31mred\u{1B}[0m"
        let sanitized = BrowserAnnotationSanitizer.sanitizeSingleLine(dangerous, maxLength: 1024)
        #expect(sanitized == "helloworldwith[31mred[0m")
    }

    @Test("strips DEL and C1 control characters")
    func stripsDELAndC1() {
        let dangerous = "before\u{7F}\u{85}\u{9B}after"
        let sanitized = BrowserAnnotationSanitizer.sanitizeSingleLine(dangerous, maxLength: 1024)
        #expect(sanitized == "beforeafter")
    }

    @Test("strips bracketed-paste end sequence so it cannot escape paste mode")
    func stripsBracketedPasteEnd() {
        let payload = "evil\u{1B}[201~ls /\u{1B}[200~"
        let sanitized = BrowserAnnotationSanitizer.sanitizeMultiLine(payload, maxLength: 1024)
        #expect(!sanitized.contains("\u{1B}"))
        #expect(sanitized == "evil[201~ls /[200~")
    }

    @Test("caps oversized inputs")
    func capsLength() {
        let big = String(repeating: "a", count: 5000)
        let sanitized = BrowserAnnotationSanitizer.sanitizeSingleLine(big, maxLength: 100)
        #expect(sanitized.count == 100)
    }

    @Test("inline-code sanitization replaces backticks to prevent code-span escape")
    func replacesBackticks() {
        let payload = "main`; rm -rf /; echo `"
        let sanitized = BrowserAnnotationSanitizer.sanitizeMarkdownInlineCode(
            payload,
            maxLength: BrowserAnnotationSanitizer.maxSelectorLength
        )
        #expect(!sanitized.contains("`"))
    }

    @Test("style value sanitizer removes css separators")
    func sanitizesStyleValue() {
        let payload = "10px; background: url(javascript:alert(1)); }<script>"
        let sanitized = BrowserAnnotationSanitizer.sanitizeStyleValue(payload)
        #expect(!sanitized.contains(";"))
        #expect(!sanitized.contains("{"))
        #expect(!sanitized.contains("}"))
        #expect(!sanitized.contains("<"))
        #expect(!sanitized.contains(">"))
    }
}
