import Foundation
import WebKit

@MainActor
final class BrowserDataStoreCache {
    static let shared = BrowserDataStoreCache()

    private var stores: [UUID: WKWebsiteDataStore] = [:]

    func store(for profileID: UUID) -> WKWebsiteDataStore {
        if let existing = stores[profileID] {
            return existing
        }
        let store = profileID == BrowserProfile.defaultID
            ? WKWebsiteDataStore.default()
            : WKWebsiteDataStore(forIdentifier: profileID)
        stores[profileID] = store
        return store
    }

    func evict(_ profileID: UUID) {
        stores[profileID] = nil
    }
}
