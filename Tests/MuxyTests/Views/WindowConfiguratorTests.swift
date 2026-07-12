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

    @Test("uses a tinted native sidebar material")
    func usesTintedNativeSidebarMaterial() {
        #expect(AppSidebarVibrancy.material == .sidebar)
        #expect(AppSidebarVibrancy.blendingMode == .behindWindow)
        #expect(AppSidebarVibrancy.state == .active)
        #expect(AppSidebarVibrancy.themeOverlayOpacity == 0.6)
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
