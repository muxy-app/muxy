import AppKit
import Testing

@testable import Muxy

@Suite("Quick terminal controller")
@MainActor
struct QuickTerminalControllerTests {
    @Test("stopping closes the terminal and blocks later shortcut triggers")
    func stopClosesTerminalAndBlocksLaterTriggers() throws {
        let shortcutBackend = QuickTerminalControllerTestShortcutBackend()
        let shortcutStore = QuickTerminalShortcutStore(
            persistence: QuickTerminalControllerTestShortcutPersistence(),
            settingsSynchronizer: {},
            canonicalizer: { $0 }
        )
        let shortcutService = QuickTerminalShortcutService(
            store: shortcutStore,
            doubleShiftBackendFactory: { shortcutBackend },
            carbonHotKeyBackendFactory: { _, _ in shortcutBackend },
            inputMonitoringAccessRequester: { false }
        )
        let surface = QuickTerminalControllerTestSurface()
        let session = QuickTerminalSession(surfaceFactory: { _ in surface })
        let controller = QuickTerminalController(
            shortcutLabelProvider: { "Option Space" },
            onOpenSettings: {},
            shortcutService: shortcutService,
            session: session,
            screenProvider: { nil },
            reduceMotionProvider: { true },
            notificationCenter: NotificationCenter(),
            workspaceNotificationCenter: NotificationCenter()
        )
        try shortcutService.start()
        defer { shortcutService.stop() }

        #expect(!controller.isVisible)

        shortcutBackend.sendTrigger()
        #expect(controller.isVisible)

        controller.stop(restoresFocus: false)

        #expect(surface.tearDownCount == 1)
        #expect(session.currentSurface == nil)
        #expect(!controller.isVisible)

        shortcutBackend.sendTrigger()
        #expect(!controller.isVisible)

        controller.applicationWillTerminate()
        #expect(surface.tearDownCount == 1)
    }
}

@MainActor
private final class QuickTerminalControllerTestShortcutBackend: QuickTerminalShortcutBackend {
    private(set) var monitoringState = QuickTerminalShortcutMonitoringState.localOnly
    private var trigger: (@MainActor () -> Void)?

    func start(trigger: @escaping @MainActor () -> Void) {
        self.trigger = trigger
    }

    func stop() {
        monitoringState = .stopped
        trigger = nil
    }

    func sendTrigger() {
        trigger?()
    }
}

private final class QuickTerminalControllerTestShortcutPersistence: QuickTerminalShortcutPersisting {
    func loadShortcut() -> QuickTerminalShortcut {
        .doubleShift
    }

    func saveShortcut(_: QuickTerminalShortcut) {}
}

@MainActor
private final class QuickTerminalControllerTestSurface: QuickTerminalSurface {
    let quickTerminalView = NSView()
    var onProcessExit: (() -> Void)?
    private(set) var tearDownCount = 0

    func applyQuickTerminalConfiguration() {}
    func setVisible(_: Bool) {}
    func setFocused(_: Bool) {}
    func notifySurfaceUnfocused() {}
    func tearDown() {
        tearDownCount += 1
    }
}
