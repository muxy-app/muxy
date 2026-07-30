import Foundation
import Testing

@testable import Muxy

@Suite("Ghostty transparent surface config")
struct GhosttyTransparentSurfaceConfigTests {
    @Test("loads the packaged override outside the managed Ghostty resources")
    func packagedOverride() throws {
        let url = try #require(GhosttyTransparentSurfaceConfig.overridesURL(bundle: .module))

        #expect(url.lastPathComponent == "transparent-surface.conf")
        #expect(url.deletingLastPathComponent().lastPathComponent == "ghostty-overrides")
        #expect(try String(contentsOf: url, encoding: .utf8) == "background-opacity = 0.00\nbackground-blur = false\n")
    }
}
