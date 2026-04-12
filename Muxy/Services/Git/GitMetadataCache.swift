import Foundation

final class GitMetadataCache: @unchecked Sendable {
    static let shared = GitMetadataCache()

    struct PRKey: Hashable {
        let repoPath: String
        let branch: String
        let headSha: String
    }

    private struct PREntry {
        let info: PRInfo?
        let storedAt: Date
    }

    private let lock = NSLock()
    private var prInfo: [PRKey: PREntry] = [:]
    private var defaultBranch: [String: String?] = [:]
    private var ghInstalled: Bool?

    private let prTTL: TimeInterval = 60

    private init() {}

    func cachedPRInfo(repoPath: String, branch: String, headSha: String) -> PRInfo?? {
        lock.lock()
        defer { lock.unlock() }
        let key = PRKey(repoPath: repoPath, branch: branch, headSha: headSha)
        guard let entry = prInfo[key] else { return nil }
        if Date().timeIntervalSince(entry.storedAt) > prTTL {
            prInfo.removeValue(forKey: key)
            return nil
        }
        return .some(entry.info)
    }

    func storePRInfo(_ info: PRInfo?, repoPath: String, branch: String, headSha: String) {
        lock.lock()
        defer { lock.unlock() }
        let key = PRKey(repoPath: repoPath, branch: branch, headSha: headSha)
        prInfo[key] = PREntry(info: info, storedAt: Date())
    }

    func invalidatePRInfo(repoPath: String, branch: String) {
        lock.lock()
        defer { lock.unlock() }
        prInfo = prInfo.filter { key, _ in
            !(key.repoPath == repoPath && key.branch == branch)
        }
    }

    func invalidatePRInfo(repoPath: String) {
        lock.lock()
        defer { lock.unlock() }
        prInfo = prInfo.filter { key, _ in key.repoPath != repoPath }
    }

    func cachedDefaultBranch(repoPath: String) -> String?? {
        lock.lock()
        defer { lock.unlock() }
        guard let value = defaultBranch[repoPath] else { return nil }
        return .some(value)
    }

    func storeDefaultBranch(_ branch: String?, repoPath: String) {
        lock.lock()
        defer { lock.unlock() }
        defaultBranch[repoPath] = branch
    }

    func cachedGhInstalled() -> Bool? {
        lock.lock()
        defer { lock.unlock() }
        return ghInstalled
    }

    func storeGhInstalled(_ installed: Bool) {
        lock.lock()
        defer { lock.unlock() }
        ghInstalled = installed
    }
}
