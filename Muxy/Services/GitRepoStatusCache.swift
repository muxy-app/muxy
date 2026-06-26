import Foundation

@MainActor
@Observable
final class GitRepoStatusCache {
    static let shared = GitRepoStatusCache()

    private var statusByKey: [String: Bool]
    private let store: CodableFileStore<[String: Bool]>

    init(
        store: CodableFileStore<[String: Bool]> = CodableFileStore(
            fileURL: MuxyFileStorage.fileURL(filename: "git-repo-status.json")
        )
    ) {
        self.store = store
        statusByKey = (try? store.load()) ?? [:]
    }

    func cachedStatus(for path: String, context: WorkspaceContext) -> Bool? {
        statusByKey[key(path: path, context: context)]
    }

    func update(path: String, context: WorkspaceContext, isGitRepo: Bool) {
        let key = key(path: path, context: context)
        guard statusByKey[key] != isGitRepo else { return }
        statusByKey[key] = isGitRepo
        try? store.save(statusByKey)
    }

    func remove(path: String, context: WorkspaceContext) {
        guard statusByKey.removeValue(forKey: key(path: path, context: context)) != nil else { return }
        try? store.save(statusByKey)
    }

    private func key(path: String, context: WorkspaceContext) -> String {
        "\(context.cacheKeyPrefix)|\(path)"
    }
}
