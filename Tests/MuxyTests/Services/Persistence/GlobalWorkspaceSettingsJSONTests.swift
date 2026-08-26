import AppKit
import Foundation
import Testing

@testable import Muxy

@Suite("Global Workspace JSON settings", .serialized)
@MainActor
struct GlobalWorkspaceSettingsJSONTests {
    @Test
    func settingsPersistThroughJSON() throws {
        let keys = [
            GlobalWorkspacePreferences.enabledKey,
            GlobalWorkspacePreferences.triggerKey,
            GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey,
            GlobalWorkspacePreferences.toggleToHideKey,
        ]
        let snapshot = GlobalWorkspaceSettingsJSONSnapshot.capture(keys: keys)
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(GlobalWorkspacePreferences.enabledKey)": true,
          "\(GlobalWorkspacePreferences.triggerKey)": "doubleOption",
          "\(GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey)": 450,
          "\(GlobalWorkspacePreferences.toggleToHideKey)": false
        }
        """)

        #expect(GlobalWorkspacePreferences.isEnabled())
        #expect(GlobalWorkspacePreferences.trigger() == .doubleOption)
        #expect(GlobalWorkspacePreferences.doubleTapIntervalMilliseconds() == 450)
        #expect(!GlobalWorkspacePreferences.toggleToHide())
    }

    @Test
    func customShortcutPersistsThroughJSON() throws {
        let keys = [
            GlobalWorkspacePreferences.triggerKey,
            GlobalWorkspacePreferences.customShortcutKey,
        ]
        let snapshot = GlobalWorkspaceSettingsJSONSnapshot.capture(keys: keys)
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText(#"""
        {
          "shortcuts.globalWorkspace": {
            "trigger": "custom",
            "customShortcut": {
              "type": "keyCombo",
              "keyCombo": { "key": "space", "modifiers": 1048576 },
              "virtualKeyCode": 49
            }
          }
        }
        """#)

        #expect(GlobalWorkspacePreferences.trigger() == .custom)
        #expect(
            GlobalWorkspacePreferences.customShortcut()
                == .keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        )
    }

    @Test
    func rejectsCustomShortcutWithoutKeyCombo() throws {
        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText(#"""
            {
              "shortcuts.globalWorkspace": {
                "trigger": "custom"
              }
            }
            """#)
        }
    }

    @Test
    func rejectsCustomShortcutConflictingWithImportedQuickTerminalShortcut() throws {
        let modifiers = NSEvent.ModifierFlags.command.rawValue

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText(#"""
            {
              "shortcuts.quickTerminal": {
                "type": "keyCombo",
                "keyCombo": { "key": "space", "modifiers": \#(modifiers) },
                "virtualKeyCode": 49
              },
              "shortcuts.globalWorkspace": {
                "trigger": "custom",
                "customShortcut": {
                  "type": "keyCombo",
                  "keyCombo": { "key": "space", "modifiers": \#(modifiers) },
                  "virtualKeyCode": 49
                }
              }
            }
            """#)
        }
    }

    @Test
    func rejectsUnsupportedTrigger() throws {
        let snapshot = GlobalWorkspaceSettingsJSONSnapshot.capture(keys: [GlobalWorkspacePreferences.triggerKey])
        defer { snapshot.restore() }

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(GlobalWorkspacePreferences.triggerKey)": "unsupported"
            }
            """)
        }
    }

    @Test
    func rejectsOutOfRangeInterval() throws {
        let snapshot = GlobalWorkspaceSettingsJSONSnapshot.capture(
            keys: [GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey]
        )
        defer { snapshot.restore() }

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey)": 5000
            }
            """)
        }
    }
}

private struct GlobalWorkspaceSettingsJSONSnapshot {
    let data: Data?
    let defaults: [String: Any]

    @MainActor
    static func capture(keys: [String]) -> GlobalWorkspaceSettingsJSONSnapshot {
        GlobalWorkspaceSettingsJSONSnapshot(
            data: try? Data(contentsOf: SettingsJSONStore.userSettingsURL),
            defaults: Dictionary(uniqueKeysWithValues: keys.map { key in
                (key, UserDefaults.standard.object(forKey: key) ?? NSNull())
            })
        )
    }

    @MainActor
    func restore() {
        if let data {
            try? data.write(to: SettingsJSONStore.userSettingsURL, options: .atomic)
        } else {
            try? FileManager.default.removeItem(at: SettingsJSONStore.userSettingsURL)
        }

        for (key, value) in defaults {
            if value is NSNull {
                UserDefaults.standard.removeObject(forKey: key)
            } else {
                UserDefaults.standard.set(value, forKey: key)
            }
        }
        GlobalWorkspaceShortcutService.shared.refresh()
    }
}
