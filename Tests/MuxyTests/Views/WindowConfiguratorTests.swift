import AppKit
import Testing

@testable import Muxy

@Suite("WindowConfigurator")
@MainActor
struct WindowConfiguratorTests {
    @Test("disallows AppKit window tabbing")
    func disallowsWindowTabbing() {
        let window = NSWindow()

        WindowConfigurator.disableWindowTabbing(for: window)

        #expect(window.tabbingMode == .disallowed)
    }
}
