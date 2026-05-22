import AppKit
import Foundation
import Testing

@testable import Muxy

@Suite("HTMLPreviewWebView")
struct HTMLPreviewWebViewTests {
    @Test("svg preview html centers and contains the image")
    @MainActor
    func svgPreviewHTMLCentersAndContainsImage() throws {
        let fileURL = FileManager.default.temporaryDirectory.appendingPathComponent("icon-\(UUID().uuidString).svg")
        try "<svg></svg>".write(to: fileURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: fileURL) }

        let html = HTMLPreviewWebView.svgPreviewHTML(
            fileURL: fileURL,
            backgroundColor: NSColor(srgbRed: 0.1, green: 0.2, blue: 0.3, alpha: 1)
        )

        #expect(html.contains("display: flex"))
        #expect(html.contains("align-items: center"))
        #expect(html.contains("justify-content: center"))
        #expect(html.contains("width: 100%"))
        #expect(html.contains("height: 100%"))
        #expect(html.contains("background: #1A334D"))
        #expect(html.contains("object-fit: contain"))
        #expect(html.contains("src=\"data:image/svg+xml;base64,"))
    }
}
