import Foundation
import Testing

@testable import Muxy

@Suite("BrowserAnnotationSender")
@MainActor
struct BrowserAnnotationSenderTests {
    @Test("renders markdown with selector, viewport, and comment")
    func rendersMarkdown() {
        let annotation = BrowserAnnotation(
            selector: "main > header h1.title",
            xpath: "/html/body/main/header/h1",
            textSnippet: "Welcome",
            rect: CGRect(x: 10, y: 20, width: 120, height: 40),
            pageURL: "https://example.com/landing",
            pageTitle: "Example",
            viewportWidth: 1440,
            viewportHeight: 900,
            comment: "Make this larger"
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)

        #expect(markdown.contains("@muxy-browser: https://example.com/landing"))
        #expect(markdown.contains("- page: Example"))
        #expect(markdown.contains("- selector: `main > header h1.title`"))
        #expect(markdown.contains("- xpath: `/html/body/main/header/h1`"))
        #expect(markdown.contains("- text: \"Welcome\""))
        #expect(markdown.contains("- bbox: (x=10, y=20, w=120, h=40)"))
        #expect(markdown.contains("- viewport: 1440×900"))
        #expect(markdown.contains("- comment: \"Make this larger\""))
    }

    @Test("includes style override lines")
    func includesStyleOverrides() {
        let override = StyleOverride(
            selector: ".btn",
            property: .backgroundColor,
            originalValue: "rgb(0, 0, 0)",
            value: "#fff"
        )
        let annotation = BrowserAnnotation(
            selector: ".btn",
            xpath: "",
            textSnippet: "Submit",
            rect: .zero,
            pageURL: "https://example.com",
            pageTitle: "",
            viewportWidth: 800,
            viewportHeight: 600,
            styleOverrides: [override]
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)
        #expect(markdown.contains("- style override: background-color: rgb(0, 0, 0) → #fff"))
    }

    @Test("strips terminal control sequences from page-supplied fields")
    func stripsTerminalControlSequences() {
        let annotation = BrowserAnnotation(
            selector: "a\u{1B}[2K",
            xpath: "x\u{07}",
            textSnippet: "evil\u{1B}[201~payload",
            rect: .zero,
            pageURL: "https://example.com\u{1B}",
            pageTitle: "title\u{07}",
            viewportWidth: 0,
            viewportHeight: 0,
            comment: "comment\u{1B}[31m\nstill ok"
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)

        #expect(!markdown.contains("\u{1B}"))
        #expect(!markdown.contains("\u{07}"))
        #expect(!markdown.contains("\u{7F}"))
    }

    @Test("escapes backticks in inline code so a page cannot break out of code spans")
    func escapesBackticksInInlineCode() {
        let annotation = BrowserAnnotation(
            selector: "main`; rm -rf /; echo `",
            xpath: "/html/body`evil`",
            textSnippet: "",
            rect: .zero,
            pageURL: "https://example.com",
            pageTitle: "",
            viewportWidth: 0,
            viewportHeight: 0
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)

        let selectorLine = markdown
            .split(whereSeparator: \.isNewline)
            .first(where: { $0.hasPrefix("- selector: ") }) ?? ""
        let xpathLine = markdown
            .split(whereSeparator: \.isNewline)
            .first(where: { $0.hasPrefix("- xpath: ") }) ?? ""
        #expect(selectorLine.filter { $0 == "`" }.count == 2)
        #expect(xpathLine.filter { $0 == "`" }.count == 2)
    }

    @Test("caps oversized comment input")
    func capsOversizedComment() {
        let annotation = BrowserAnnotation(
            selector: ".x",
            xpath: "",
            textSnippet: "",
            rect: .zero,
            pageURL: "https://example.com",
            pageTitle: "",
            viewportWidth: 0,
            viewportHeight: 0,
            comment: String(repeating: "a", count: BrowserAnnotationSanitizer.maxCommentLength + 256)
        )
        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)
        #expect(markdown.count < BrowserAnnotationSanitizer.maxCommentLength + 1024)
    }
}
