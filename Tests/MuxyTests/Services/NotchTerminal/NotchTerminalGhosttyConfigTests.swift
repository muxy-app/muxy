import Testing

@testable import Muxy

@Suite("Notch terminal Ghostty config")
struct NotchTerminalGhosttyConfigTests {
    @Test("leaves background composition to the native material stack")
    func transparentBackgroundOverride() {
        #expect(NotchTerminalGhosttyConfig.configText() == "background-opacity = 0.00\nbackground-blur = false\n")
    }
}
