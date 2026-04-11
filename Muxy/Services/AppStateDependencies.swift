import Foundation

@MainActor
protocol ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID?
    func saveActiveProjectID(_ id: UUID?)
    func loadActiveWorktreeIDs() -> [UUID: UUID]
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID])
}

@MainActor
final class UserDefaultsActiveProjectSelectionStore: ActiveProjectSelectionStoring {
    private let defaults: UserDefaults
    private let projectKey: String
    private let worktreesKey: String

    init(
        defaults: UserDefaults = .standard,
        projectKey: String = "muxy.activeProjectID",
        worktreesKey: String = "muxy.activeWorktreeIDs"
    ) {
        self.defaults = defaults
        self.projectKey = projectKey
        self.worktreesKey = worktreesKey
    }

    func loadActiveProjectID() -> UUID? {
        guard let idString = defaults.string(forKey: projectKey) else { return nil }
        return UUID(uuidString: idString)
    }

    func saveActiveProjectID(_ id: UUID?) {
        defaults.set(id?.uuidString, forKey: projectKey)
    }

    func loadActiveWorktreeIDs() -> [UUID: UUID] {
        guard let raw = defaults.dictionary(forKey: worktreesKey) as? [String: String] else { return [:] }
        var result: [UUID: UUID] = [:]
        for (projectString, worktreeString) in raw {
            guard let projectID = UUID(uuidString: projectString),
                  let worktreeID = UUID(uuidString: worktreeString)
            else { continue }
            result[projectID] = worktreeID
        }
        return result
    }

    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) {
        let encoded = Dictionary(uniqueKeysWithValues: ids.map { ($0.key.uuidString, $0.value.uuidString) })
        defaults.set(encoded, forKey: worktreesKey)
    }
}

@MainActor
protocol TerminalViewRemoving {
    func removeView(for paneID: UUID)
    func needsConfirmQuit(for paneID: UUID) -> Bool
}
