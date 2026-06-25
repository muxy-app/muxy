import Foundation

@MainActor
@Observable
final class GitRepoStatusCache {
    static let shared = GitRepoStatusCache()

    private var statusByPath: [String: Bool]
    private let store: CodableFileStore<[String: Bool]>

    init(
        store: CodableFileStore<[String: Bool]> = CodableFileStore(
            fileURL: MuxyFileStorage.fileURL(filename: "git-repo-status.json")
        )
    ) {
        self.store = store
        statusByPath = (try? store.load()) ?? [:]
    }

    func cachedStatus(for path: String) -> Bool? {
        statusByPath[path]
    }

    func update(path: String, isGitRepo: Bool) {
        guard statusByPath[path] != isGitRepo else { return }
        statusByPath[path] = isGitRepo
        try? store.save(statusByPath)
    }
}
