import AppKit
import Testing

@testable import Muxy

@Suite("PasteboardFileURLs")
@MainActor
struct PasteboardFileURLsTests {
    @Test("reads file URLs written by a Finder style copy")
    func readsFileURLs() {
        let pasteboard = NSPasteboard(name: NSPasteboard.Name(UUID().uuidString))
        pasteboard.clearContents()
        let urls = [
            URL(fileURLWithPath: "/tmp/report.pdf"),
            URL(fileURLWithPath: "/tmp/photo.png"),
        ]
        pasteboard.writeObjects(urls as [NSURL])

        #expect(PasteboardFileURLs.urls(in: pasteboard) == urls)
    }

    @Test("ignores remote URLs")
    func ignoresRemoteURLs() throws {
        let pasteboard = NSPasteboard(name: NSPasteboard.Name(UUID().uuidString))
        pasteboard.clearContents()
        let remote = try #require(URL(string: "https://example.com/report.pdf"))
        pasteboard.writeObjects([remote as NSURL])

        #expect(PasteboardFileURLs.urls(in: pasteboard).isEmpty)
    }

    @Test("returns nothing for a plain text pasteboard")
    func ignoresPlainText() {
        let pasteboard = NSPasteboard(name: NSPasteboard.Name(UUID().uuidString))
        pasteboard.clearContents()
        pasteboard.setString("/tmp/report.pdf", forType: .string)

        #expect(PasteboardFileURLs.urls(in: pasteboard).isEmpty)
    }
}
