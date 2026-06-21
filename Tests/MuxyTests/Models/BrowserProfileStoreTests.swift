import Foundation
import Testing

@testable import Muxy

@Suite("BrowserProfileStore", .serialized)
@MainActor
struct BrowserProfileStoreTests {
    private func makeStore(initial: [BrowserProfile] = []) -> BrowserProfileStore {
        UserDefaults.standard.removeObject(forKey: BrowserPreferences.defaultProfileIDKey)
        return BrowserProfileStore(persistence: InMemoryBrowserProfilePersistence(initial: initial))
    }

    @Test("seeds a default profile when empty")
    func seedsDefault() {
        let store = makeStore()
        #expect(store.profiles.contains { $0.id == BrowserProfile.defaultID })
        #expect(store.profiles.first { $0.isDefault }?.name == BrowserProfile.defaultName)
    }

    @Test("does not duplicate default profile when already present")
    func keepsExistingDefault() {
        let store = makeStore(initial: [.default])
        #expect(store.profiles.filter { $0.id == BrowserProfile.defaultID }.count == 1)
    }

    @Test("add appends a profile")
    func addProfile() {
        let store = makeStore()
        let count = store.profiles.count
        store.add(name: "Work")
        #expect(store.profiles.count == count + 1)
        #expect(store.profiles.contains { $0.name == "Work" })
    }

    @Test("rename updates the name")
    func renameProfile() {
        let store = makeStore()
        let profile = store.add(name: "Work")
        store.rename(id: profile.id, to: "Personal")
        #expect(store.profiles.first { $0.id == profile.id }?.name == "Personal")
    }

    @Test("delete removes a non-default profile")
    func deleteProfile() {
        let store = makeStore()
        let profile = store.add(name: "Work")
        store.remove(id: profile.id)
        #expect(!store.profiles.contains { $0.id == profile.id })
    }

    @Test("default profile cannot be deleted")
    func cannotDeleteDefault() {
        let store = makeStore()
        store.remove(id: BrowserProfile.defaultID)
        #expect(store.profiles.contains { $0.id == BrowserProfile.defaultID })
    }

    @Test("setDefault can be changed and reset back to Default")
    func defaultCanBeReset() {
        let store = makeStore()
        let profile = store.add(name: "Work")
        store.setDefault(id: profile.id)
        #expect(store.defaultProfileID == profile.id)
        store.setDefault(id: BrowserProfile.defaultID)
        #expect(store.defaultProfileID == BrowserProfile.defaultID)
    }

    @Test("deleting the default-selected profile resets to Default")
    func deletingDefaultSelectionResets() {
        let store = makeStore()
        let profile = store.add(name: "Work")
        store.setDefault(id: profile.id)
        store.remove(id: profile.id)
        #expect(store.defaultProfileID == BrowserProfile.defaultID)
    }
}
