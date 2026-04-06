import Foundation

@MainActor
protocol ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID?
    func saveActiveProjectID(_ id: UUID?)
}

@MainActor
final class UserDefaultsActiveProjectSelectionStore: ActiveProjectSelectionStoring {
    private let defaults: UserDefaults
    private let key: String

    init(defaults: UserDefaults = .standard, key: String = "muxy.activeProjectID") {
        self.defaults = defaults
        self.key = key
    }

    func loadActiveProjectID() -> UUID? {
        guard let idString = defaults.string(forKey: key) else { return nil }
        return UUID(uuidString: idString)
    }

    func saveActiveProjectID(_ id: UUID?) {
        defaults.set(id?.uuidString, forKey: key)
    }
}

@MainActor
protocol TerminalViewRemoving {
    func removeView(for paneID: UUID)
    func needsConfirmQuit(for paneID: UUID) -> Bool
}
