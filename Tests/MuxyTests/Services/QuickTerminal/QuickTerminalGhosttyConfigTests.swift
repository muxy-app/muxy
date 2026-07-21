import Foundation
import Testing

@testable import Muxy

@Suite("Quick terminal Ghostty config")
struct QuickTerminalGhosttyConfigTests {
    @Test("loads the packaged override outside the managed Ghostty resources")
    func packagedOverride() throws {
        let url = try #require(QuickTerminalGhosttyConfig.overridesURL(bundle: .module))

        #expect(url.lastPathComponent == "ghostty.conf")
        #expect(url.deletingLastPathComponent().lastPathComponent == "quick-terminal")
        #expect(try String(contentsOf: url, encoding: .utf8) == "background-opacity = 0.00\nbackground-blur = false\n")
    }
}
