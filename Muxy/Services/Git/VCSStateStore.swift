import Foundation

@MainActor
@Observable
final class VCSStateStore {
    static let shared = VCSStateStore()

    private(set) var states: [String: VCSTabState] = [:]

    private init() {}

    func state(for path: String) -> VCSTabState {
        let key = Self.canonicalize(path)
        if let existing = states[key] { return existing }
        let state = VCSTabState(projectPath: path)
        states[key] = state
        return state
    }

    func cachedState(for path: String) -> VCSTabState? {
        states[Self.canonicalize(path)]
    }

    func remove(path: String) {
        states.removeValue(forKey: Self.canonicalize(path))
    }

    private static func canonicalize(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath().path
    }
}
