import Foundation
import os
import WebKit

private let logger = Logger(subsystem: "app.muxy", category: "BrowserProfileStore")

@MainActor
@Observable
final class BrowserProfileStore {
    private(set) var profiles: [BrowserProfile] = []
    private(set) var defaultProfileID: UUID = BrowserProfile.defaultID
    private let persistence: any BrowserProfilePersisting

    init(persistence: any BrowserProfilePersisting) {
        self.persistence = persistence
        load()
    }

    func profile(id: UUID?) -> BrowserProfile? {
        guard let id else { return nil }
        return profiles.first(where: { $0.id == id })
    }

    @discardableResult
    func add(name: String) -> BrowserProfile {
        let profile = BrowserProfile(name: trimmedName(name, fallback: "Profile"))
        profiles.append(profile)
        save()
        return profile
    }

    func rename(id: UUID, to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              let index = profiles.firstIndex(where: { $0.id == id })
        else { return }
        profiles[index].name = trimmed
        save()
    }

    func setDefault(id: UUID) {
        guard profiles.contains(where: { $0.id == id }) else { return }
        defaultProfileID = id
        BrowserPreferences.defaultProfileID = id
    }

    func remove(id: UUID) {
        guard id != BrowserProfile.defaultID else { return }
        profiles.removeAll { $0.id == id }
        if defaultProfileID == id {
            setDefault(id: BrowserProfile.defaultID)
        }
        save()
        purgeData(for: id)
    }

    func clearData(for id: UUID) async {
        let dataStore = BrowserDataStoreCache.shared.store(for: id)
        let types = WKWebsiteDataStore.allWebsiteDataTypes()
        await dataStore.removeData(ofTypes: types, modifiedSince: .distantPast)
    }

    private func purgeData(for id: UUID) {
        BrowserDataStoreCache.shared.evict(id)
        WKWebsiteDataStore.remove(forIdentifier: id) { error in
            if let error {
                logger.error("Failed to purge browser profile data: \(error)")
            }
        }
    }

    private func trimmedName(_ name: String, fallback: String) -> String {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? fallback : trimmed
    }

    private func save() {
        do {
            try persistence.saveProfiles(profiles)
        } catch {
            logger.error("Failed to save browser profiles: \(error)")
        }
    }

    private func load() {
        do {
            profiles = try persistence.loadProfiles()
        } catch {
            logger.error("Failed to load browser profiles: \(error)")
        }
        if !profiles.contains(where: { $0.id == BrowserProfile.defaultID }) {
            profiles.insert(.default, at: 0)
            save()
        }
        let storedDefault = BrowserPreferences.defaultProfileID
        defaultProfileID = profiles.contains(where: { $0.id == storedDefault })
            ? storedDefault
            : BrowserProfile.defaultID
    }
}
