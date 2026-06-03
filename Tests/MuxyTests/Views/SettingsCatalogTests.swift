import Foundation
import Testing
@testable import Muxy

@Suite("SettingsCatalog")
@MainActor
struct SettingsCatalogTests {
    private func makeDefaults() -> UserDefaults {
        let suiteName = "SettingsRouteSelectionStoreTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }

    @Test
    func searchFindsSettingsByAliasAndDescription() {
        let results = SettingsCatalog.matchingItems(query: "hotkeys")

        #expect(results.contains { $0.category == .shortcuts && $0.title == "App Shortcuts" })
    }

    @Test
    func categoryMatchingUsesCatalogItems() {
        #expect(SettingsCatalog.categoryMatches(.editor, query: "line numbers"))
        #expect(!SettingsCatalog.categoryMatches(.mobile, query: "line numbers"))
    }

    @Test
    func settingsUseWorkflowCategories() {
        #expect(SettingsCatalog.items.contains { $0.key == ProjectPickerPreferences.storageKey && $0.category == .projects })
        #expect(SettingsCatalog.items.contains { $0.key == GeneralSettingsKeys.autoCopyTerminalSelection && $0.category == .terminal })
        #expect(SettingsCatalog.items.contains { $0.key == RecordingPreferences.languageKey && $0.category == .voice })
    }

    @Test
    func desktopNotificationsAreRegisteredAndSearchable() {
        #expect(SettingsCatalog.items.contains {
            $0.key == NotificationSettings.Key.desktopEnabled && $0.category == .notifications
        })
        #expect(SettingsCatalog.matchingItems(query: "desktop").contains {
            $0.key == NotificationSettings.Key.desktopEnabled
        })
    }

    @Test
    func jsonEditableItemsHaveDefaults() {
        #expect(!SettingsCatalog.jsonEditableItems.isEmpty)
        #expect(SettingsCatalog.jsonEditableItems.allSatisfy { $0.defaultValue != nil })
    }

    @Test
    func jsonEditableItemsIncludeEditorSettings() {
        #expect(SettingsCatalog.items.contains { $0.key.hasPrefix("editor.") })
        #expect(SettingsCatalog.jsonEditableItems.contains { $0.key == "editor.defaultEditor" })
        #expect(SettingsCatalog.jsonEditableItems.contains { $0.key == "editor.richInputLineHeightMultiplier" })
    }

    @Test
    func settingsRoutesRoundTripStoredIDs() throws {
        #expect(SettingsRoute(storedID: "builtin.terminal") == .builtin(.terminal))
        #expect(SettingsRoute(storedID: "ext.com.example.tool") == .ext("com.example.tool"))
        #expect(SettingsRoute(storedID: "builtin.missing") == nil)
        #expect(SettingsRoute(storedID: "ext.") == nil)
    }

    @Test
    func selectedSettingsRoutePersists() throws {
        let defaults = makeDefaults()

        #expect(SettingsRouteSelectionStore.load(defaults: defaults) == .builtin(.general))

        SettingsRouteSelectionStore.save(.builtin(.editor), defaults: defaults)
        #expect(defaults.string(forKey: SettingsRouteSelectionStore.storageKey) == "builtin.editor")
        #expect(SettingsRouteSelectionStore.load(defaults: defaults) == .builtin(.editor))

        defaults.set("invalid", forKey: SettingsRouteSelectionStore.storageKey)
        #expect(SettingsRouteSelectionStore.load(defaults: defaults) == .builtin(.general))
    }
}
