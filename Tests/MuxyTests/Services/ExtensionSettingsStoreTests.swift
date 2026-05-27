import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("ExtensionSettingsStore")
struct ExtensionSettingsStoreTests {
    @Test("round-trips primitive values per extension and key")
    func roundTripsPrimitives() {
        let defaults = makeIsolatedDefaults()
        let store = ExtensionSettingsStore(defaults: defaults)

        store.setValue(.bool(true), extensionID: "ext-a", key: "enabled")
        store.setValue(.string("https://x"), extensionID: "ext-a", key: "endpoint")
        store.setValue(.number(42), extensionID: "ext-b", key: "count")

        #expect(store.value(extensionID: "ext-a", key: "enabled") == .bool(true))
        #expect(store.value(extensionID: "ext-a", key: "endpoint") == .string("https://x"))
        #expect(store.value(extensionID: "ext-b", key: "count") == .number(42))
        #expect(store.value(extensionID: "ext-a", key: "count") == nil)
    }

    @Test("nil clears the stored value")
    func nilClears() {
        let defaults = makeIsolatedDefaults()
        let store = ExtensionSettingsStore(defaults: defaults)

        store.setValue(.bool(true), extensionID: "ext", key: "k")
        #expect(store.value(extensionID: "ext", key: "k") == .bool(true))

        store.setValue(nil, extensionID: "ext", key: "k")
        #expect(store.value(extensionID: "ext", key: "k") == nil)
    }

    @Test("effectiveValue falls back to manifest default")
    func effectiveValueFallback() {
        let defaults = makeIsolatedDefaults()
        let store = ExtensionSettingsStore(defaults: defaults)
        let entry = ExtensionSettingEntry(
            key: "endpoint",
            title: "Endpoint",
            description: nil,
            type: .string,
            defaultValue: .string("default")
        )

        #expect(store.effectiveValue(extensionID: "ext", entry: entry) == .string("default"))

        store.setValue(.string("override"), extensionID: "ext", key: "endpoint")
        #expect(store.effectiveValue(extensionID: "ext", entry: entry) == .string("override"))
    }

    @Test("clearAll removes only this extension's keys")
    func clearAllIsolated() {
        let defaults = makeIsolatedDefaults()
        let store = ExtensionSettingsStore(defaults: defaults)

        store.setValue(.bool(true), extensionID: "ext-a", key: "k1")
        store.setValue(.bool(true), extensionID: "ext-a", key: "k2")
        store.setValue(.bool(true), extensionID: "ext-b", key: "k1")

        store.clearAll(extensionID: "ext-a")
        #expect(store.value(extensionID: "ext-a", key: "k1") == nil)
        #expect(store.value(extensionID: "ext-a", key: "k2") == nil)
        #expect(store.value(extensionID: "ext-b", key: "k1") == .bool(true))
    }

    @Test("storageKey is prefixed and includes extensionID")
    func storageKeyFormat() {
        #expect(ExtensionSettingsStore.storageKey(extensionID: "ext", key: "k") == "muxy.ext.ext.k")
    }

    private func makeIsolatedDefaults() -> UserDefaults {
        let suiteName = "ExtensionSettingsStoreTests.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Failed to create isolated UserDefaults")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
