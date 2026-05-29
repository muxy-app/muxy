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

    @Test("rejects untitled window requests")
    func rejectsUntitledWindowRequests() {
        let delegate = AppDelegate()

        #expect(!delegate.applicationShouldOpenUntitledFile(NSApplication.shared))
    }

    @Test("allows auxiliary windows to close")
    func allowsAuxiliaryWindowsToClose() {
        let delegate = AppDelegate()
        let window = NSWindow()

        #expect(delegate.windowShouldClose(window))
    }
}
