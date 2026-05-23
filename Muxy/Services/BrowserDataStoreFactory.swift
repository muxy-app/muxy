import Foundation
import WebKit

@MainActor
enum BrowserDataStoreFactory {
    private static let activeStores = NSHashTable<WKWebsiteDataStore>.weakObjects()

    static func dataStore() -> WKWebsiteDataStore {
        let store: WKWebsiteDataStore = BrowserPreferences.persistData
            ? .default()
            : .nonPersistent()
        activeStores.add(store)
        return store
    }

    static func clearAllBrowsingData() async {
        await clearPersistentData()
        await clearActiveStores()
    }

    static func clearPersistentData() async {
        await clear(store: .default())
    }

    private static func clearActiveStores() async {
        let stores = activeStores.allObjects
        for store in stores where store !== WKWebsiteDataStore.default() {
            await clear(store: store)
        }
    }

    private static func clear(store: WKWebsiteDataStore) async {
        let types = WKWebsiteDataStore.allWebsiteDataTypes()
        let records = await store.dataRecords(ofTypes: types)
        await store.removeData(ofTypes: types, for: records)
    }
}
