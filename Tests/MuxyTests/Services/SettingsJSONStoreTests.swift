import CoreGraphics
import Foundation
import Testing

@testable import Muxy

@Suite("SettingsJSONStore", .serialized)
@MainActor
struct SettingsJSONStoreTests {
    @Test
    func saveAppliesKnownSettingsAndPreservesUnknownKeys() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [MobileServerService.portKey])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("{\"unknown.setting\":{\"nested\":true},\"\(MobileServerService.portKey)\":4242}")

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(UserDefaults.standard.integer(forKey: MobileServerService.portKey) == 4242)
        #expect(savedText.contains("\"unknown.setting\""))
        #expect(savedText.contains("  \"nested\" : true"))
        #expect(savedText.hasSuffix("\n"))
    }

    @Test
    func applyUserSettingsFileAppliesImportedSettings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [MobileServerService.portKey])
        defer { snapshot.restore() }

        UserDefaults.standard.set(1234, forKey: MobileServerService.portKey)
        try Data("{\"\(MobileServerService.portKey)\":4242}".utf8).write(to: SettingsJSONStore.userSettingsURL, options: .atomic)

        try SettingsJSONStore.applyUserSettingsFile()

        #expect(UserDefaults.standard.integer(forKey: MobileServerService.portKey) == 4242)
    }

    @Test
    func invalidKnownValueDoesNotWriteOrApplySettings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [MobileServerService.portKey])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)
        UserDefaults.standard.set(4242, forKey: MobileServerService.portKey)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(MobileServerService.portKey)": 0
            }
            """)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(savedText == originalText)
        #expect(UserDefaults.standard.integer(forKey: MobileServerService.portKey) == 4242)
    }

    @Test
    func invalidSpecialValueDoesNotWriteSettings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "ai.providers": []
            }
            """)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(savedText == originalText)
    }

    @Test
    func invalidAppShortcutsDoNotReplaceBindings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        let originalBindings = KeyBindingStore.shared.bindings
        defer {
            KeyBindingStore.shared.replaceBindings(originalBindings)
            snapshot.restore()
        }

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "shortcuts.app": {
                "unknownAction": {}
              }
            }
            """)
        }

        #expect(KeyBindingStore.shared.bindings.count == originalBindings.count)
        for binding in originalBindings {
            let current = KeyBindingStore.shared.bindings.first { $0.action == binding.action }
            #expect(current?.combo == binding.combo)
        }
    }

    @Test
    func appShortcutsAllowUnassignedBindings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        let originalBindings = KeyBindingStore.shared.bindings
        defer {
            KeyBindingStore.shared.replaceBindings(originalBindings)
            snapshot.restore()
        }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "shortcuts.app": {
            "refreshWorktrees": {
              "key": "",
              "modifiers": 0
            }
          }
        }
        """)

        #expect(KeyBindingStore.shared.combo(for: .refreshWorktrees) == KeyCombo(key: "", modifiers: 0))
    }

    @Test
    func omittedKnownSettingsRemainUnchanged() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [MobileServerService.portKey])
        defer { snapshot.restore() }

        UserDefaults.standard.set(4242, forKey: MobileServerService.portKey)

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "unknown.setting": true
        }
        """)

        #expect(UserDefaults.standard.integer(forKey: MobileServerService.portKey) == 4242)
    }

    @Test
    func tabHeaderWidthPersistsZeroAsFullWidth() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [TabWidthPreferences.maxWidthKey])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(TabWidthPreferences.maxWidthKey)": 0
        }
        """)

        #expect(UserDefaults.standard.double(forKey: TabWidthPreferences.maxWidthKey) == 0)
        #expect(TabWidthPreferences.effectiveMaxWidth(from: 0) == nil)
    }

    @Test
    func tabHeaderWidthPersistsPixelCap() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [TabWidthPreferences.maxWidthKey])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(TabWidthPreferences.maxWidthKey)": 200
        }
        """)

        #expect(UserDefaults.standard.double(forKey: TabWidthPreferences.maxWidthKey) == 200)
        #expect(TabWidthPreferences.effectiveMaxWidth(from: 200) == CGFloat(200))
    }

    @Test
    func tabHeaderWidthTreatsMaximumAsFullWidth() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [TabWidthPreferences.maxWidthKey])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(TabWidthPreferences.maxWidthKey)": 400
        }
        """)

        #expect(UserDefaults.standard.double(forKey: TabWidthPreferences.maxWidthKey) == 400)
        #expect(TabWidthPreferences.effectiveMaxWidth(from: 400) == nil)
    }

    @Test
    func tabHeaderWidthRemovesKeyForNull() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [TabWidthPreferences.maxWidthKey])
        defer { snapshot.restore() }

        UserDefaults.standard.set(200, forKey: TabWidthPreferences.maxWidthKey)

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(TabWidthPreferences.maxWidthKey)": null
        }
        """)

        #expect(UserDefaults.standard.object(forKey: TabWidthPreferences.maxWidthKey) == nil)
    }

    @Test
    func tabHeaderWidthAcceptsArbitraryConfigPixelValue() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [TabWidthPreferences.maxWidthKey])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(TabWidthPreferences.maxWidthKey)": 320
        }
        """)

        #expect(UserDefaults.standard.double(forKey: TabWidthPreferences.maxWidthKey) == 320)
        #expect(TabWidthPreferences.effectiveMaxWidth(from: 320) == CGFloat(320))
    }

    @Test
    func tabHeaderWidthRejectsNegativeValues() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [TabWidthPreferences.maxWidthKey])
        defer { snapshot.restore() }

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(TabWidthPreferences.maxWidthKey)": -1
            }
            """)
        }
    }

    @Test
    func tabHeaderWidthSliderRoundTrips() {
        #expect(TabWidthPreferences.sliderValue(from: TabWidthPreferences.defaultMaxWidth) == 200)
        #expect(TabWidthPreferences.sliderValue(from: 0) == TabWidthPreferences.maxMaxWidth)
        #expect(TabWidthPreferences.sliderValue(from: 320) == 320)
        #expect(TabWidthPreferences.sliderValue(from: 50) == TabWidthPreferences.minMaxWidth)

        #expect(TabWidthPreferences.storedValue(forSlider: TabWidthPreferences.maxMaxWidth) == 0)
        #expect(TabWidthPreferences.storedValue(forSlider: 200) == 200)
        #expect(TabWidthPreferences.storedValue(forSlider: 50) == TabWidthPreferences.minMaxWidth)
    }

    @Test
    func prettifiedSettingsTextSortsAndFormatsJSONObject() throws {
        let text = try SettingsJSONStore.prettifiedSettingsText("{\"z\":1,\"a\":{\"b\":true}}")

        #expect(text == """
        {
          "a" : {
            "b" : true
          },
          "z" : 1
        }

        """)
    }

    @Test
    func saveAppliesEditorSettings() throws {
        let settings = EditorSettings.shared
        let originalStrategy = settings.richInputImageStrategy
        let originalFontFamily = settings.richInputFontFamily
        let originalMultiplier = settings.richInputLineHeightMultiplier
        defer {
            settings.richInputImageStrategy = originalStrategy
            settings.richInputFontFamily = originalFontFamily
            settings.richInputLineHeightMultiplier = originalMultiplier
        }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "editor.richInputImageStrategy": "inlinePath",
          "editor.richInputFontFamily": "Menlo",
          "editor.richInputLineHeightMultiplier": 1.5
        }
        """)

        #expect(settings.richInputImageStrategy == .inlinePath)
        #expect(settings.richInputFontFamily == "Menlo")
        #expect(settings.richInputLineHeightMultiplier == 1.5)
    }

    @Test
    func systemSettingsIncludeAllBackedSettings() throws {
        let data = Data(SettingsJSONStore.systemSettingsText.utf8)
        let object = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])

        for item in SettingsCatalog.jsonEditableItems {
            #expect(object.keys.contains(item.key))
        }
        #expect(object.keys.contains("shortcuts.app"))
        #expect(object.keys.contains("shortcuts.customCommands"))
        #expect(object.keys.contains("ai.providers"))
        #expect(object.keys.contains("mobile.approvedDevices"))
    }

    @Test
    func syncWritesCurrentSettingsToUserJSON() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [MobileServerService.portKey])
        defer { snapshot.restore() }

        UserDefaults.standard.set(4242, forKey: MobileServerService.portKey)
        SettingsJSONStore.syncUserSettingsFileWithCurrentSettings()

        let data = try Data(contentsOf: SettingsJSONStore.userSettingsURL)
        let object = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])

        #expect(object[MobileServerService.portKey] as? Int == 4242)
        #expect(object.keys.contains("shortcuts.app"))
    }
}

private struct SettingsJSONStoreSnapshot {
    let data: Data?
    let defaults: [String: Any]

    @MainActor
    static func capture(keys: [String]) -> SettingsJSONStoreSnapshot {
        SettingsJSONStoreSnapshot(
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
    }
}
