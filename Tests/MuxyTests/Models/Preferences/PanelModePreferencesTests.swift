import Foundation
import Testing

@testable import Muxy

@Suite("Panel mode preferences")
@MainActor
struct PanelModePreferencesTests {
    @Test("persists mode independently for each panel")
    func persistsModePerPanel() throws {
        let suiteName = "PanelModePreferencesTests-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = PanelModePreferences(defaults: defaults)
        let host = PanelHost(modePreferences: preferences)

        host.open("ext:files:files", at: .right, mode: .pinned)
        host.setMode(.floating, for: "ext:files:files")
        host.close("ext:files:files")
        host.open("ext:files:files", at: .right, mode: .pinned)
        host.open("ext:git:changes", at: .bottom, mode: .pinned)

        #expect(host.placement(for: "ext:files:files")?.mode == .floating)
        #expect(host.placement(for: "ext:git:changes")?.mode == .pinned)

        let restoredHost = PanelHost(modePreferences: PanelModePreferences(defaults: defaults))
        restoredHost.open("ext:files:files", at: .right, mode: .pinned)
        #expect(restoredHost.placement(for: "ext:files:files")?.mode == .floating)

        let fixedModeHost = PanelHost(modePreferences: PanelModePreferences(defaults: defaults))
        fixedModeHost.open("ext:files:files", at: .right, mode: .pinned, usesPreferredMode: false)
        #expect(fixedModeHost.placement(for: "ext:files:files")?.mode == .pinned)
    }

    @Test("migrates the legacy Rich Input floating preference")
    func migratesLegacyRichInputPreference() throws {
        let suiteName = "PanelModePreferencesTests-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set(false, forKey: RichInputPreferences.panelFloatingKey)
        let preferences = PanelModePreferences(defaults: defaults)

        let mode = preferences.mode(for: BuiltinPanel.richInput, default: .floating)

        #expect(mode == .pinned)
        #expect(defaults.string(forKey: preferences.storageKey(for: BuiltinPanel.richInput)) == "pinned")
    }
}
