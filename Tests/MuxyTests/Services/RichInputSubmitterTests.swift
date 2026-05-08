import Foundation
import Testing

@testable import Muxy

@Suite("RichInputSubmitter.tokenize")
struct RichInputSubmitterTests {
    @Test("returns single text segment when no images")
    func textOnly() {
        let segments = RichInputSubmitter.tokenize(text: "hello world", images: [])
        #expect(segments == [.text("hello world")])
    }

    @Test("returns empty when text is empty and no images")
    func empty() {
        let segments = RichInputSubmitter.tokenize(text: "", images: [])
        #expect(segments.isEmpty)
    }

    @Test("splits at image placeholder")
    func singleImage() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "before [Image 1] after", images: [url])
        #expect(segments == [.text("before "), .image(url), .text(" after")])
    }

    @Test("preserves order across multiple images")
    func multipleImages() {
        let a = URL(fileURLWithPath: "/tmp/a.png")
        let b = URL(fileURLWithPath: "/tmp/b.png")
        let segments = RichInputSubmitter.tokenize(
            text: "look [Image 1] then [Image 2]",
            images: [a, b]
        )
        #expect(segments == [
            .text("look "),
            .image(a),
            .text(" then "),
            .image(b),
        ])
    }

    @Test("treats out-of-range placeholder as plain text")
    func unknownPlaceholder() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "hi [Image 7]", images: [url])
        #expect(segments == [.text("hi [Image 7]")])
    }

    @Test("placeholder at start with no leading text")
    func leadingPlaceholder() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "[Image 1] tail", images: [url])
        #expect(segments == [.image(url), .text(" tail")])
    }

    @Test("placeholder at end with no trailing text")
    func trailingPlaceholder() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "head [Image 1]", images: [url])
        #expect(segments == [.text("head "), .image(url)])
    }

    @Test("only-image text resolves to single image segment")
    func onlyImage() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "[Image 1]", images: [url])
        #expect(segments == [.image(url)])
    }
}
