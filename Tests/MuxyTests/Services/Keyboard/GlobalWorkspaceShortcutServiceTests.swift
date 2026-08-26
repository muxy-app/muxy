import AppKit
import Testing

@testable import Muxy

@Suite("GlobalWorkspaceShortcutService")
@MainActor
struct GlobalWorkspaceShortcutServiceTests {
    @Test("custom shortcut uses the Carbon hot key backend")
    func customShortcutUsesCarbonHotKeyBackend() throws {
        let suiteName = "GlobalWorkspaceShortcutServiceTests.customShortcut"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }

        defaults.set(true, forKey: GlobalWorkspacePreferences.enabledKey)
        try GlobalWorkspacePreferences.setCustomShortcut(
            .keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49),
            defaults: defaults
        )

        var doubleModifierCreated = false
        var carbonHotKeyCreated = false
        let service = GlobalWorkspaceShortcutService(
            defaults: defaults,
            doubleModifierBackendFactory: { _, _ in
                doubleModifierCreated = true
                return GlobalWorkspaceTestBackend(state: .systemWide)
            },
            carbonHotKeyBackendFactory: { _, _ in
                carbonHotKeyCreated = true
                return GlobalWorkspaceTestBackend(state: .carbonHotKey)
            }
        )

        service.start()

        #expect(carbonHotKeyCreated)
        #expect(!doubleModifierCreated)
        #expect(service.monitoringState == .carbonHotKey)

        service.stop()
    }
}

@MainActor
private final class GlobalWorkspaceTestBackend: QuickTerminalShortcutBackend {
    let monitoringState: QuickTerminalShortcutMonitoringState

    init(state: QuickTerminalShortcutMonitoringState) {
        monitoringState = state
    }

    func start(trigger _: @escaping @MainActor () -> Void) throws {}

    func stop() {}
}
