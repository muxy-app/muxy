import Foundation
import Testing

@testable import Muxy

@Suite("Quick terminal Ghostty config")
struct QuickTerminalGhosttyConfigTests {
    @Test("leaves background composition to the native material stack")
    func transparentBackgroundOverride() throws {
        let url = try #require(QuickTerminalGhosttyConfig.overridesURL())

        #expect(try String(contentsOf: url, encoding: .utf8) == "background-opacity = 0.00\nbackground-blur = false\n")
    }
}
