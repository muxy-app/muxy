import Foundation
import Testing

@testable import Muxy

@Suite("Ghostty defaults config")
struct GhosttyDefaultsConfigTests {
    @Test("loads the packaged defaults outside the managed Ghostty resources")
    func packagedDefaults() throws {
        let url = try #require(GhosttyDefaultsConfig.defaultsURL(bundle: .module))

        #expect(url.lastPathComponent == "muxy-defaults.conf")
        #expect(url.deletingLastPathComponent().lastPathComponent == "ghostty-overrides")
        #expect(try String(contentsOf: url, encoding: .utf8) == "window-padding-color = extend\n")
    }
}
