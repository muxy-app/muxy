import Foundation
import Testing

@testable import Muxy

@Suite("Notch terminal Ghostty config")
struct NotchTerminalGhosttyConfigTests {
    @Test("leaves background composition to the native material stack")
    func transparentBackgroundOverride() throws {
        let url = try #require(NotchTerminalGhosttyConfig.overridesURL())

        #expect(try String(contentsOf: url, encoding: .utf8) == "background-opacity = 0.00\nbackground-blur = false\n")
    }
}
