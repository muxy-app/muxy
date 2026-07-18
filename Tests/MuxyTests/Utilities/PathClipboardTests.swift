import AppKit
import Foundation
import Testing

@testable import Muxy

@Suite("PathClipboard")
@MainActor
struct PathClipboardTests {
    @Test("copy replaces pasteboard contents with the path")
    func copyReplacesPasteboardContents() {
        let pasteboard = NSPasteboard.withUniqueName()
        let staleType = NSPasteboard.PasteboardType("app.muxy.tests.stale")
        defer { pasteboard.releaseGlobally() }

        pasteboard.setData(Data([1]), forType: staleType)

        PathClipboard.copy("/Users/test/Projects/Muxy", to: pasteboard)

        #expect(pasteboard.string(forType: .string) == "/Users/test/Projects/Muxy")
        #expect(pasteboard.data(forType: staleType) == nil)
    }
}
