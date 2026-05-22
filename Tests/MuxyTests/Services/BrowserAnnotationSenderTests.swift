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
}
