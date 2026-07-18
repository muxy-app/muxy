import Foundation

final class GitMetadataCache: @unchecked Sendable {
    static let shared = GitMetadataCache()

    struct PRKey: Hashable {
        let context: WorkspaceContext
        let repoPath: String
        let branch: String
        let headSha: String
    }

    private struct PREntry {
        let info: GitRepositoryService.PRInfo?
        let storedAt: Date
    }

    struct ReadKey: Hashable {
        let repoPath: String
        let endpoint: String
        let params: String
    }

    private struct ReadEntry {
        let value: Any
        let signature: String
        let storedAt: Date
    }

    private let lock = NSLock()
    private var prInfo: [PRKey: PREntry] = [:]
    private var defaultBranch: [String: String?] = [:]
    private var ghInstalled: Bool?
    private var remoteWebURL: [String: URL?] = [:]
    private var verifiedGitRepo: Set<String> = []
    private var reads: [ReadKey: ReadEntry] = [:]
    private var readOrder: [ReadKey] = []

    private let prTTL: TimeInterval = 300
    private let readTTL: TimeInterval = 5
    private let readCapacity = 128

    private init() {}

    func cachedRead<T>(_ key: ReadKey, signature: String) -> T? {
        lock.lock()
        defer { lock.unlock() }
        guard let entry = reads[key] else { return nil }
        if entry.signature != signature || Date().timeIntervalSince(entry.storedAt) > readTTL {
            removeRead(key)
            return nil
        }
        return entry.value as? T
    }

    func storeRead(_ value: Any, key: ReadKey, signature: String) {
        lock.lock()
        defer { lock.unlock() }
        if reads[key] == nil {
            readOrder.append(key)
        }
        reads[key] = ReadEntry(value: value, signature: signature, storedAt: Date())
        while readOrder.count > readCapacity {
            removeRead(readOrder[0])
        }
    }

    func invalidateReads(repoPath: String) {
        lock.lock()
        defer { lock.unlock() }
        for key in readOrder where key.repoPath == repoPath {
            reads.removeValue(forKey: key)
        }
        readOrder.removeAll { $0.repoPath == repoPath }
    }

    private func removeRead(_ key: ReadKey) {
        reads.removeValue(forKey: key)
        readOrder.removeAll { $0 == key }
    }

    func cachedPRInfo(
        context: WorkspaceContext,
        repoPath: String,
        branch: String,
        headSha: String
    ) -> GitRepositoryService.PRInfo?? {
        lock.lock()
        defer { lock.unlock() }
        let key = PRKey(context: context, repoPath: repoPath, branch: branch, headSha: headSha)
        guard let entry = prInfo[key] else { return nil }
        if Date().timeIntervalSince(entry.storedAt) > prTTL {
            prInfo.removeValue(forKey: key)
            return nil
        }
        return .some(entry.info)
    }

    func storePRInfo(
        _ info: GitRepositoryService.PRInfo?,
        context: WorkspaceContext,
        repoPath: String,
        branch: String,
        headSha: String
    ) {
        lock.lock()
        defer { lock.unlock() }
        let key = PRKey(context: context, repoPath: repoPath, branch: branch, headSha: headSha)
        prInfo[key] = PREntry(info: info, storedAt: Date())
    }

    func invalidatePRInfo(context: WorkspaceContext, repoPath: String, branch: String) {
        lock.lock()
        defer { lock.unlock() }
        prInfo = prInfo.filter { key, _ in
            !(key.context == context && key.repoPath == repoPath && key.branch == branch)
        }
    }

    func invalidatePRInfo(context: WorkspaceContext, repoPath: String) {
        lock.lock()
        defer { lock.unlock() }
        prInfo = prInfo.filter { key, _ in key.context != context || key.repoPath != repoPath }
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

    func cachedRemoteWebURL(repoPath: String) -> URL?? {
        lock.lock()
        defer { lock.unlock() }
        guard let value = remoteWebURL[repoPath] else { return nil }
        return .some(value)
    }

    func storeRemoteWebURL(_ url: URL?, repoPath: String) {
        lock.lock()
        defer { lock.unlock() }
        remoteWebURL[repoPath] = url
    }

    func invalidateRemoteWebURL(repoPath: String) {
        lock.lock()
        defer { lock.unlock() }
        remoteWebURL.removeValue(forKey: repoPath)
    }

    func isVerifiedGitRepo(repoPath: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return verifiedGitRepo.contains(repoPath)
    }

    func markVerifiedGitRepo(repoPath: String) {
        lock.lock()
        defer { lock.unlock() }
        verifiedGitRepo.insert(repoPath)
    }
}
