import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "BrowserHistoryStore")

@MainActor
@Observable
final class BrowserHistoryStore {
    static let maxEntries = 2000
    static let suggestionLimit = 8

    private(set) var entries: [BrowserHistoryEntry] = []
    private let persistence: any BrowserHistoryPersisting
    @ObservationIgnored private var saveTask: Task<Void, Never>?

    init(persistence: any BrowserHistoryPersisting) {
        self.persistence = persistence
        load()
    }

    func record(url: URL, title: String?, profileID: UUID) {
        guard let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" else { return }
        let urlString = url.absoluteString
        let cleanedTitle = title?.trimmingCharacters(in: .whitespacesAndNewlines)

        if let index = entries.firstIndex(where: { $0.profileID == profileID && $0.url == urlString }) {
            var entry = entries.remove(at: index)
            entry.lastVisited = Date()
            entry.visitCount += 1
            if let cleanedTitle, !cleanedTitle.isEmpty {
                entry.title = cleanedTitle
            }
            entries.insert(entry, at: 0)
        } else {
            let entry = BrowserHistoryEntry(
                profileID: profileID,
                url: urlString,
                title: cleanedTitle?.isEmpty == false ? cleanedTitle : nil,
                lastVisited: Date()
            )
            entries.insert(entry, at: 0)
            trimIfNeeded()
        }
        scheduleSave()
    }

    func updateTitle(_ title: String?, for url: URL, profileID: UUID) {
        let cleaned = title?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let cleaned, !cleaned.isEmpty else { return }
        let urlString = url.absoluteString
        guard let index = entries.firstIndex(where: { $0.profileID == profileID && $0.url == urlString }),
              entries[index].title != cleaned
        else { return }
        entries[index].title = cleaned
        scheduleSave()
    }

    func suggestions(for query: String, profileID: UUID, limit: Int = suggestionLimit) -> [BrowserHistoryEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let scoped = entries.filter { $0.profileID == profileID }
        guard !trimmed.isEmpty else {
            return Array(scoped.prefix(limit))
        }
        let matched = scoped.filter { $0.matches(query: trimmed) }
        let ranked = matched.sorted { lhs, rhs in
            if lhs.visitCount != rhs.visitCount {
                return lhs.visitCount > rhs.visitCount
            }
            return lhs.lastVisited > rhs.lastVisited
        }
        return Array(ranked.prefix(limit))
    }

    func clear(profileID: UUID) {
        entries.removeAll { $0.profileID == profileID }
        saveImmediately()
    }

    func saveImmediately() {
        saveTask?.cancel()
        saveTask = nil
        saveToDisk(entries)
    }

    private func trimIfNeeded() {
        guard entries.count > Self.maxEntries else { return }
        entries = Array(entries.prefix(Self.maxEntries))
    }

    private func scheduleSave() {
        saveTask?.cancel()
        let snapshot = entries
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled else { return }
            self?.saveToDisk(snapshot)
        }
    }

    private func saveToDisk(_ entries: [BrowserHistoryEntry]) {
        do {
            try persistence.saveEntries(entries)
        } catch {
            logger.error("Failed to save browser history: \(error)")
        }
    }

    private func load() {
        do {
            let loaded = try persistence.loadEntries()
            entries = Array(loaded.sorted { $0.lastVisited > $1.lastVisited }.prefix(Self.maxEntries))
        } catch {
            logger.error("Failed to load browser history: \(error)")
        }
    }
}
