import Testing
@testable import Muxy

@Suite("SettingsCatalog")
@MainActor
struct SettingsCatalogTests {
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
}
