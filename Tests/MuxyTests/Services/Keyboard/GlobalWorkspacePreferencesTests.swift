import Foundation
import Testing

@testable import Muxy

@Suite("GlobalWorkspacePreferences")
struct GlobalWorkspacePreferencesTests {
    @Test("defaults keep the global workspace opt-in")
    func defaultsAreOptIn() throws {
        let suiteName = "GlobalWorkspacePreferencesTests.defaults"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }

        #expect(!GlobalWorkspacePreferences.isEnabled(defaults: defaults))
        #expect(GlobalWorkspacePreferences.trigger(defaults: defaults) == .doubleCommand)
        #expect(GlobalWorkspacePreferences.doubleTapIntervalMilliseconds(defaults: defaults) == 300)
        #expect(GlobalWorkspacePreferences.toggleToHide(defaults: defaults))
    }

    @Test(
        "supported modifier triggers round trip",
        arguments: [
            GlobalWorkspaceTrigger.doubleCommand,
            GlobalWorkspaceTrigger.doubleControl,
            GlobalWorkspaceTrigger.doubleOption,
        ]
    )
    func supportedModifierTriggersRoundTrip(trigger: GlobalWorkspaceTrigger) throws {
        let suiteName = "GlobalWorkspacePreferencesTests.trigger.\(trigger.rawValue)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }

        defaults.set(trigger.rawValue, forKey: GlobalWorkspacePreferences.triggerKey)

        #expect(GlobalWorkspacePreferences.trigger(defaults: defaults) == trigger)
    }

    @Test("custom shortcut is a supported trigger")
    func customShortcutIsSupported() {
        #expect(GlobalWorkspaceTrigger(rawValue: "custom") != nil)
    }

    @Test("custom shortcut persists with its trigger")
    @MainActor
    func customShortcutPersistsWithItsTrigger() throws {
        let suiteName = "GlobalWorkspacePreferencesTests.customShortcut"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let shortcut = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "a", command: true),
            virtualKeyCode: 0
        )
        try GlobalWorkspacePreferences.setCustomShortcut(shortcut, defaults: defaults)

        #expect(GlobalWorkspacePreferences.trigger(defaults: defaults) == .custom)
        #expect(GlobalWorkspacePreferences.customShortcut(defaults: defaults) == shortcut)
    }

    @Test("invalid trigger falls back to double command")
    func invalidTriggerFallsBack() throws {
        let suiteName = "GlobalWorkspacePreferencesTests.invalidTrigger"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }

        defaults.set("unsupported", forKey: GlobalWorkspacePreferences.triggerKey)

        #expect(GlobalWorkspacePreferences.trigger(defaults: defaults) == .doubleCommand)
    }

    @Test("stored interval is clamped")
    func intervalIsClamped() throws {
        let suiteName = "GlobalWorkspacePreferencesTests.interval"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }

        defaults.set(50, forKey: GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey)
        #expect(
            GlobalWorkspacePreferences.doubleTapIntervalMilliseconds(defaults: defaults)
                == GlobalWorkspacePreferences.minimumDoubleTapIntervalMilliseconds
        )

        defaults.set(1500, forKey: GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey)
        #expect(
            GlobalWorkspacePreferences.doubleTapIntervalMilliseconds(defaults: defaults)
                == GlobalWorkspacePreferences.maximumDoubleTapIntervalMilliseconds
        )
    }

    @Test("reset removes all stored overrides")
    func resetRemovesOverrides() throws {
        let suiteName = "GlobalWorkspacePreferencesTests.reset"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }

        defaults.set(true, forKey: GlobalWorkspacePreferences.enabledKey)
        defaults.set(GlobalWorkspaceTrigger.doubleOption.rawValue, forKey: GlobalWorkspacePreferences.triggerKey)
        defaults.set(700, forKey: GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey)
        defaults.set(false, forKey: GlobalWorkspacePreferences.toggleToHideKey)
        defaults.set(
            try JSONEncoder().encode(
                QuickTerminalShortcut.keyCombo(KeyCombo(key: "a", command: true), virtualKeyCode: 0)
            ),
            forKey: GlobalWorkspacePreferences.customShortcutKey
        )

        GlobalWorkspacePreferences.resetToDefaults(defaults: defaults)

        #expect(!GlobalWorkspacePreferences.isEnabled(defaults: defaults))
        #expect(GlobalWorkspacePreferences.trigger(defaults: defaults) == .doubleCommand)
        #expect(GlobalWorkspacePreferences.doubleTapIntervalMilliseconds(defaults: defaults) == 300)
        #expect(GlobalWorkspacePreferences.toggleToHide(defaults: defaults))
        #expect(GlobalWorkspacePreferences.customShortcut(defaults: defaults) == nil)
    }
}
