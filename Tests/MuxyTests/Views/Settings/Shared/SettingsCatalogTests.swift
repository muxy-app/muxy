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
    func quickTerminalShortcutIsSearchable() {
        let item = SettingsCatalog.matchingItems(query: "double shift").first {
            $0.key == "shortcuts.quickTerminal"
        }

        #expect(item?.category == .quickTerminal)
        #expect(item?.section == "Shortcut")
        #expect(!SettingsCatalog.jsonEditableItems.contains { $0.key == "shortcuts.quickTerminal" })
    }

    @Test
    func quickTerminalSettingsAreSearchableAndJSONEditable() {
        let quickTerminalItems = SettingsCatalog.items.filter { $0.category == .quickTerminal }

        #expect(quickTerminalItems.allSatisfy { $0.category == .quickTerminal })
        #expect(quickTerminalItems.contains { $0.key == QuickTerminalPreferences.enabledKey })
        #expect(quickTerminalItems.contains { $0.key == QuickTerminalSizePreferences.widthKey })
        #expect(quickTerminalItems.contains { $0.key == QuickTerminalSizePreferences.heightKey })
        #expect(quickTerminalItems.contains { $0.key == QuickTerminalAppearancePreferences.transparencyKey })
        #expect(quickTerminalItems.contains { $0.key == QuickTerminalAppearancePreferences.blurIntensityKey })
        #expect(SettingsCatalog.matchingItems(query: "terminal size").contains {
            $0.key == QuickTerminalSizePreferences.widthKey
        })
        #expect(SettingsCatalog.matchingItems(query: "disable").contains {
            $0.key == QuickTerminalPreferences.enabledKey
        })
        #expect(SettingsCatalog.jsonEditableItems.contains {
            $0.key == QuickTerminalPreferences.enabledKey
        })
        #expect(SettingsCatalog.jsonEditableItems.contains {
            $0.key == QuickTerminalSizePreferences.heightKey
        })
        #expect(SettingsCatalog.matchingItems(query: "glass").contains {
            $0.key == QuickTerminalAppearancePreferences.transparencyKey
        })
        #expect(SettingsCatalog.jsonEditableItems.contains {
            $0.key == QuickTerminalAppearancePreferences.blurIntensityKey
        })
        #expect(SettingsCatalog.matchingItems(query: "vibrancy").contains {
            $0.key == QuickTerminalAppearancePreferences.blurIntensityKey
        })
        #expect(SettingsCatalog.sectionMatches(query: "terminal size", category: .quickTerminal, section: "Size"))
        #expect(SettingsCatalog.sectionMatches(query: "vibrancy", category: .quickTerminal, section: "Appearance"))
    }

    @Test
    func categoryMatchingUsesCatalogItems() {
        #expect(SettingsCatalog.categoryMatches(.richInput, query: "rich input"))
        #expect(!SettingsCatalog.categoryMatches(.mobile, query: "rich input"))
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
    func worktreeListSettingsAreRegisteredAndSearchable() {
        #expect(SettingsCatalog.items.contains {
            $0.key == WorktreeListPreferences.showUnreadIndicatorKey && $0.category == .appearance
        })
        #expect(SettingsCatalog.items.contains {
            $0.key == WorktreeListPreferences.orderByMRUKey && $0.category == .appearance
        })
        #expect(SettingsCatalog.matchingItems(query: "mru").contains {
            $0.key == WorktreeListPreferences.orderByMRUKey
        })
        #expect(SettingsCatalog.matchingItems(query: "unread").contains {
            $0.key == WorktreeListPreferences.showUnreadIndicatorKey
        })
    }

    @Test
    func worktreePathTemplateIsRegisteredAndSearchable() {
        #expect(SettingsCatalog.jsonEditableItems.contains {
            $0.key == GeneralSettingsKeys.defaultWorktreePathTemplate
        })
        #expect(SettingsCatalog.matchingItems(query: "relative branch").contains {
            $0.key == GeneralSettingsKeys.defaultWorktreePathTemplate
        })
    }

    @Test
    func repositoryAIActionsAreRegisteredAndSearchable() {
        for action in RepositoryAIAction.allCases {
            let provider = SettingsCatalog.items.first { $0.key == action.providerKey }
            let prompt = SettingsCatalog.items.first { $0.key == action.promptKey }

            #expect(provider?.category == .ai)
            #expect(prompt?.category == .ai)
            #expect(provider?.defaultValue == AnyHashable(RepositoryAIActionPreferences.automaticProviderID))
            #expect(prompt?.defaultValue == AnyHashable(action.defaultPrompt))
        }
        #expect(SettingsCatalog.matchingItems(query: "github").contains {
            $0.key == RepositoryAIAction.createPullRequest.promptKey
        })
        #expect(SettingsCatalog.matchingItems(query: "push").contains {
            $0.key == RepositoryAIAction.commit.promptKey
        })
    }

    @Test
    func sidebarBackgroundIsRegisteredAndSearchable() throws {
        let item = try #require(SettingsCatalog.items.first {
            $0.key == AppBackgroundStyle.storageKey
        })

        #expect(item.category == .appearance)
        #expect(item.section == "Sidebar")
        #expect(item.defaultValue == AnyHashable(AppBackgroundStyle.defaultValue.rawValue))
        #expect(SettingsCatalog.matchingItems(query: "vibrancy").contains {
            $0.key == AppBackgroundStyle.storageKey
        })
        #expect(SettingsCatalog.jsonEditableItems.contains {
            $0.key == AppBackgroundStyle.storageKey
        })
    }

    @Test
    func jsonEditableItemsHaveDefaults() {
        #expect(!SettingsCatalog.jsonEditableItems.isEmpty)
        #expect(SettingsCatalog.jsonEditableItems.allSatisfy { $0.defaultValue != nil })
    }

    @Test
    func jsonEditableItemsIncludeRichInputSettings() {
        #expect(SettingsCatalog.items.contains { $0.key.hasPrefix("editor.") })
        #expect(SettingsCatalog.jsonEditableItems.contains { $0.key == "editor.richInputImageStrategy" })
        #expect(SettingsCatalog.jsonEditableItems.contains { $0.key == "editor.richInputLineHeightMultiplier" })
    }

    @Test
    func settingsRoutesRoundTripStoredIDs() throws {
        #expect(SettingsRoute(storedID: "builtin.terminal") == .builtin(.terminal))
        #expect(SettingsRoute(storedID: "builtin.quickTerminal") == .builtin(.quickTerminal))
        #expect(SettingsRoute(storedID: "ext.com.example.tool") == .ext("com.example.tool"))
        #expect(SettingsRoute(storedID: "builtin.missing") == nil)
        #expect(SettingsRoute(storedID: "ext.") == nil)
    }

    @Test
    func selectedSettingsRoutePersists() throws {
        let defaults = makeDefaults()

        #expect(SettingsRouteSelectionStore.load(defaults: defaults) == .builtin(.general))

        SettingsRouteSelectionStore.save(.builtin(.richInput), defaults: defaults)
        #expect(defaults.string(forKey: SettingsRouteSelectionStore.storageKey) == "builtin.richInput")
        #expect(SettingsRouteSelectionStore.load(defaults: defaults) == .builtin(.richInput))

        defaults.set("invalid", forKey: SettingsRouteSelectionStore.storageKey)
        #expect(SettingsRouteSelectionStore.load(defaults: defaults) == .builtin(.general))
    }
}
