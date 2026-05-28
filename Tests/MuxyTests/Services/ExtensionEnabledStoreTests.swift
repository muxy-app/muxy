import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionEnabledStore")
struct ExtensionEnabledStoreTests {
    @Test("defaults to enabled when no value is stored")
    func defaultsToEnabledWhenUnset() {
        let defaults = makeIsolatedDefaults()
        #expect(ExtensionEnabledStore.isEnabled(extensionID: "ext-a", defaults: defaults))
    }

    @Test("persists disabled state and reads it back")
    func persistsDisabledState() {
        let defaults = makeIsolatedDefaults()
        ExtensionEnabledStore.setEnabled(false, extensionID: "ext-a", defaults: defaults)
        #expect(!ExtensionEnabledStore.isEnabled(extensionID: "ext-a", defaults: defaults))
    }

    @Test("persists enabled state explicitly and reads it back")
    func persistsEnabledStateExplicitly() {
        let defaults = makeIsolatedDefaults()
        ExtensionEnabledStore.setEnabled(false, extensionID: "ext-a", defaults: defaults)
        ExtensionEnabledStore.setEnabled(true, extensionID: "ext-a", defaults: defaults)
        #expect(ExtensionEnabledStore.isEnabled(extensionID: "ext-a", defaults: defaults))
    }

    @Test("clear removes the stored override")
    func clearResetsToDefault() {
        let defaults = makeIsolatedDefaults()
        ExtensionEnabledStore.setEnabled(false, extensionID: "ext-a", defaults: defaults)
        ExtensionEnabledStore.clear(extensionID: "ext-a", defaults: defaults)
        #expect(ExtensionEnabledStore.isEnabled(extensionID: "ext-a", defaults: defaults))
    }

    @Test("isolates state per extension id")
    func isolatesPerExtension() {
        let defaults = makeIsolatedDefaults()
        ExtensionEnabledStore.setEnabled(false, extensionID: "ext-a", defaults: defaults)
        #expect(!ExtensionEnabledStore.isEnabled(extensionID: "ext-a", defaults: defaults))
        #expect(ExtensionEnabledStore.isEnabled(extensionID: "ext-b", defaults: defaults))
    }

    @Test("hasOverride reflects whether a value has been stored")
    func hasOverrideReflectsStorage() {
        let defaults = makeIsolatedDefaults()
        #expect(!ExtensionEnabledStore.hasOverride(extensionID: "ext-a", defaults: defaults))
        ExtensionEnabledStore.setEnabled(false, extensionID: "ext-a", defaults: defaults)
        #expect(ExtensionEnabledStore.hasOverride(extensionID: "ext-a", defaults: defaults))
        ExtensionEnabledStore.clear(extensionID: "ext-a", defaults: defaults)
        #expect(!ExtensionEnabledStore.hasOverride(extensionID: "ext-a", defaults: defaults))
    }

    private func makeIsolatedDefaults() -> UserDefaults {
        let suiteName = "ExtensionEnabledStoreTests.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Failed to create isolated UserDefaults")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
