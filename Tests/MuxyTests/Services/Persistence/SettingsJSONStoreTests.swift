import AppKit
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
    func staticWorktreeTemplateDoesNotWriteOrApplySettings() throws {
        let key = GeneralSettingsKeys.defaultWorktreePathTemplate
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [key])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"
        let originalTemplate = "../{base-dir}.{branch}"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)
        UserDefaults.standard.set(originalTemplate, forKey: key)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(key)": "/tmp/worktrees"
            }
            """)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(savedText == originalText)
        #expect(UserDefaults.standard.string(forKey: key) == originalTemplate)
    }

    @Test
    func emptyWorktreeTemplateRemainsUnset() throws {
        let key = GeneralSettingsKeys.defaultWorktreePathTemplate
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [key])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(key)": ""
        }
        """)

        #expect(UserDefaults.standard.string(forKey: key) == "")
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
    func invalidQuickTerminalShortcutDoesNotWriteSettings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "shortcuts.quickTerminal": {
                "type": "keyCombo",
                "keyCombo": {
                  "key": "space",
                  "modifiers": 0
                }
              }
            }
            """)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(savedText == originalText)
    }

    @Test("noncanonical Quick Terminal shortcuts do not write settings", arguments: [
        ("SPACE", NSEvent.ModifierFlags.command.rawValue),
        ("space", NSEvent.ModifierFlags.command.rawValue | NSEvent.ModifierFlags.capsLock.rawValue),
    ])
    func noncanonicalQuickTerminalShortcutDoesNotWriteSettings(key: String, modifiers: UInt) throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "shortcuts.quickTerminal": {
                "type": "keyCombo",
                "keyCombo": {
                  "key": "\(key)",
                  "modifiers": \(modifiers)
                },
                "virtualKeyCode": 49
              }
            }
            """)
        }

        #expect(try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8) == originalText)
    }

    @Test
    func conflictingQuickTerminalShortcutDoesNotWriteSettings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"
        let modifiers = NSEvent.ModifierFlags.command.rawValue

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "shortcuts.app": {
                "newTab": {
                  "key": "space",
                  "modifiers": \(modifiers)
                }
              },
              "shortcuts.quickTerminal": {
                "type": "keyCombo",
                "keyCombo": {
                  "key": "space",
                  "modifiers": \(modifiers)
                }
              }
            }
            """)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(savedText == originalText)
    }

    @Test
    func conflictingQuickTerminalRegistrationIdentityDoesNotWriteSettings() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"
        let modifiers = NSEvent.ModifierFlags.command.rawValue

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "shortcuts.app": {
                "newTab": {
                  "key": "space",
                  "modifiers": \(modifiers)
                }
              },
              "shortcuts.quickTerminal": {
                "type": "keyCombo",
                "keyCombo": {
                  "key": "q",
                  "modifiers": \(modifiers)
                },
                "virtualKeyCode": 49
              }
            }
            """)
        }

        #expect(try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8) == originalText)
    }

    @Test
    func failedQuickTerminalRegistrationRestoresSettingsFile() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"
        let modifiers: UInt = [
            NSEvent.ModifierFlags.command,
            .control,
            .option,
            .shift,
        ].reduce(0) { $0 | $1.rawValue }
        var attemptedShortcut: QuickTerminalShortcut?

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONApplyTestError.registrationFailed) {
            try SettingsJSONStore.saveUserSettingsText(
                """
                {
                  "shortcuts.quickTerminal": {
                    "type": "keyCombo",
                    "keyCombo": {
                      "key": "space",
                      "modifiers": \(modifiers)
                    },
                    "virtualKeyCode": 49
                  }
                }
                """,
                quickTerminalShortcutUpdater: {
                    attemptedShortcut = $0
                    throw SettingsJSONApplyTestError.registrationFailed
                }
            )
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(attemptedShortcut?.keyCombo == KeyCombo(key: "space", modifiers: modifiers))
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

    @Test("disabling Quick Terminal is applied after shortcut changes")
    func quickTerminalDisableFollowsShortcutUpdate() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [QuickTerminalPreferences.enabledKey])
        defer { snapshot.restore() }
        var events: [String] = []

        try SettingsJSONStore.saveUserSettingsText(
            """
            {
              "\(QuickTerminalPreferences.enabledKey)": false,
              "shortcuts.quickTerminal": {
                "type": "doubleShift"
              }
            }
            """,
            quickTerminalShortcutUpdater: { _ in events.append("shortcut") },
            quickTerminalEnabledUpdater: { events.append("enabled \($0)") }
        )

        #expect(events == ["shortcut", "enabled false"])
    }

    @Test("enabling Quick Terminal is applied after shortcut changes")
    func quickTerminalEnableFollowsShortcutUpdate() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [QuickTerminalPreferences.enabledKey])
        defer { snapshot.restore() }
        var events: [String] = []

        try SettingsJSONStore.saveUserSettingsText(
            """
            {
              "\(QuickTerminalPreferences.enabledKey)": true,
              "shortcuts.quickTerminal": {
                "type": "doubleShift"
              }
            }
            """,
            quickTerminalShortcutUpdater: { _ in events.append("shortcut") },
            quickTerminalEnabledUpdater: { events.append("enabled \($0)") }
        )

        #expect(events == ["shortcut", "enabled true"])
    }

    @Test("shortcut failure does not change Quick Terminal enabled state")
    func quickTerminalShortcutFailurePreservesEnabledState() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [QuickTerminalPreferences.enabledKey])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"
        var isEnabled = true

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONApplyTestError.registrationFailed) {
            try SettingsJSONStore.saveUserSettingsText(
                """
                {
                  "\(QuickTerminalPreferences.enabledKey)": false,
                  "shortcuts.quickTerminal": {
                    "type": "doubleShift"
                  }
                }
                """,
                quickTerminalShortcutUpdater: { _ in
                    throw SettingsJSONApplyTestError.registrationFailed
                },
                quickTerminalEnabledUpdater: { isEnabled = $0 }
            )
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)
        #expect(savedText == originalText)
        #expect(isEnabled)
    }

    @Test("invalid Quick Terminal enabled value does not write settings")
    func invalidQuickTerminalEnabledValueIsRejected() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [QuickTerminalPreferences.enabledKey])
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(QuickTerminalPreferences.enabledKey)": "false"
            }
            """)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)
        #expect(savedText == originalText)
    }

    @Test("null Quick Terminal enabled value resets to the default")
    func nullQuickTerminalEnabledValueResetsToDefault() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [QuickTerminalPreferences.enabledKey])
        defer { snapshot.restore() }
        QuickTerminalPreferences.setEnabled(false)

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(QuickTerminalPreferences.enabledKey)": null
        }
        """)

        #expect(UserDefaults.standard.object(forKey: QuickTerminalPreferences.enabledKey) == nil)
        #expect(QuickTerminalPreferences.isEnabled())
    }

    @Test
    func quickTerminalSizePersistsWithinAllowedRange() throws {
        let keys = [QuickTerminalSizePreferences.widthKey, QuickTerminalSizePreferences.heightKey]
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: keys)
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(QuickTerminalSizePreferences.widthKey)": 960,
          "\(QuickTerminalSizePreferences.heightKey)": 600
        }
        """)

        #expect(QuickTerminalSizePreferences.width() == 960)
        #expect(QuickTerminalSizePreferences.height() == 600)
    }

    @Test
    func invalidQuickTerminalSizeDoesNotWriteOrApplySettings() throws {
        let keys = [QuickTerminalSizePreferences.widthKey, QuickTerminalSizePreferences.heightKey]
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: keys)
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)
        UserDefaults.standard.set(720, forKey: QuickTerminalSizePreferences.widthKey)
        UserDefaults.standard.set(430, forKey: QuickTerminalSizePreferences.heightKey)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(QuickTerminalSizePreferences.widthKey)": 320,
              "\(QuickTerminalSizePreferences.heightKey)": 600
            }
            """)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(savedText == originalText)
        #expect(QuickTerminalSizePreferences.width() == 720)
        #expect(QuickTerminalSizePreferences.height() == 430)
    }

    @Test
    func quickTerminalAppearancePersistsWithinAllowedValues() throws {
        let keys = [
            QuickTerminalAppearancePreferences.transparencyKey,
            QuickTerminalAppearancePreferences.blurIntensityKey,
        ]
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: keys)
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(QuickTerminalAppearancePreferences.transparencyKey)": 40,
          "\(QuickTerminalAppearancePreferences.blurIntensityKey)": 86
        }
        """)

        #expect(QuickTerminalAppearancePreferences.transparency() == 40)
        #expect(QuickTerminalAppearancePreferences.blurIntensity() == 86)
    }

    @Test(arguments: [0, 100])
    func quickTerminalBlurIntensityAcceptsEndpoints(_ intensity: Int) throws {
        let key = QuickTerminalAppearancePreferences.blurIntensityKey
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [key])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("{\"\(key)\":\(intensity)}")

        #expect(QuickTerminalAppearancePreferences.blurIntensity() == intensity)
    }

    @Test(arguments: [
        "{\"\(QuickTerminalAppearancePreferences.transparencyKey)\": 80}",
        "{\"\(QuickTerminalAppearancePreferences.transparencyKey)\": false}",
        "{\"\(QuickTerminalAppearancePreferences.blurIntensityKey)\": -1}",
        "{\"\(QuickTerminalAppearancePreferences.blurIntensityKey)\": 101}",
        "{\"\(QuickTerminalAppearancePreferences.blurIntensityKey)\": true}",
        "{\"\(QuickTerminalAppearancePreferences.blurIntensityKey)\": false}",
    ])
    func invalidQuickTerminalAppearanceDoesNotWriteOrApplySettings(settings: String) throws {
        let keys = [
            QuickTerminalAppearancePreferences.transparencyKey,
            QuickTerminalAppearancePreferences.blurIntensityKey,
        ]
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: keys)
        defer { snapshot.restore() }
        let originalText = "{\"unchanged\":true}\n"

        try originalText.write(to: SettingsJSONStore.userSettingsURL, atomically: true, encoding: .utf8)
        UserDefaults.standard.set(18, forKey: QuickTerminalAppearancePreferences.transparencyKey)
        UserDefaults.standard.set(70, forKey: QuickTerminalAppearancePreferences.blurIntensityKey)

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText(settings)
        }

        let savedText = try String(contentsOf: SettingsJSONStore.userSettingsURL, encoding: .utf8)

        #expect(savedText == originalText)
        #expect(QuickTerminalAppearancePreferences.transparency() == 18)
        #expect(QuickTerminalAppearancePreferences.blurIntensity() == 70)
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
    func sidebarBackgroundPersistsSupportedValue() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [AppBackgroundStyle.storageKey])
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(AppBackgroundStyle.storageKey)": "solid"
        }
        """)

        #expect(UserDefaults.standard.string(forKey: AppBackgroundStyle.storageKey) == "solid")
    }

    @Test
    func sidebarBackgroundRejectsUnsupportedValue() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [AppBackgroundStyle.storageKey])
        defer { snapshot.restore() }

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(AppBackgroundStyle.storageKey)": "transparent"
            }
            """)
        }
    }

    @Test
    func repositoryAIActionSettingsAcceptSupportedProvidersAndPrompts() throws {
        let keys = [RepositoryAIAction.commit.providerKey, RepositoryAIAction.commit.promptKey]
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: keys)
        defer { snapshot.restore() }

        try SettingsJSONStore.saveUserSettingsText("""
        {
          "\(RepositoryAIAction.commit.providerKey)": "codex",
          "\(RepositoryAIAction.commit.promptKey)": "Use Conventional Commits"
        }
        """)

        #expect(UserDefaults.standard.string(forKey: RepositoryAIAction.commit.providerKey) == "codex")
        #expect(UserDefaults.standard.string(forKey: RepositoryAIAction.commit.promptKey) == "Use Conventional Commits")
    }

    @Test
    func repositoryAIActionSettingsRejectUnsupportedProviders() throws {
        let key = RepositoryAIAction.createPullRequest.providerKey
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [key])
        defer { snapshot.restore() }

        #expect(throws: SettingsJSONError.self) {
            try SettingsJSONStore.saveUserSettingsText("""
            {
              "\(key)": "unsupported-provider"
            }
            """)
        }
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
        let quickTerminalShortcut = try #require(object["shortcuts.quickTerminal"] as? [String: Any])
        #expect(quickTerminalShortcut["type"] as? String == "unassigned")
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
        #expect(object.keys.contains("shortcuts.quickTerminal"))
    }

    @Test
    func syncSkipsAnIdenticalUserSettingsFile() throws {
        let snapshot = SettingsJSONStoreSnapshot.capture(keys: [])
        defer { snapshot.restore() }

        try Data("{}".utf8).write(to: SettingsJSONStore.userSettingsURL, options: .atomic)

        #expect(SettingsJSONStore.syncUserSettingsFileWithCurrentSettings())
        #expect(!SettingsJSONStore.syncUserSettingsFileWithCurrentSettings())
    }
}

private enum SettingsJSONApplyTestError: Error {
    case registrationFailed
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
