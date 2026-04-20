import Foundation

@MainActor
@Observable
final class DiffCache {
    struct LoadedDiff {
        let rows: [DiffDisplayRow]
        let additions: Int
        let deletions: Int
        let truncated: Bool
    }

    private(set) var diffsByPath: [String: LoadedDiff] = [:]
    private(set) var loadingPaths: Set<String> = []
    private(set) var errorsByPath: [String: String] = [:]

    @ObservationIgnored private var accessOrder: [String] = []
    @ObservationIgnored nonisolated(unsafe) private var tasks: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private let cap: Int

    init(cap: Int = 50) {
        self.cap = cap
    }

    func diff(for filePath: String) -> LoadedDiff? {
        diffsByPath[filePath]
    }

    func isLoading(_ filePath: String) -> Bool {
        loadingPaths.contains(filePath)
    }

    func error(for filePath: String) -> String? {
        errorsByPath[filePath]
    }

    func hasDiff(for filePath: String) -> Bool {
        diffsByPath[filePath] != nil
    }

    func markLoading(_ filePath: String) {
        loadingPaths.insert(filePath)
        errorsByPath[filePath] = nil
    }

    func store(_ diff: LoadedDiff, for filePath: String, pinnedPaths: Set<String>) {
        diffsByPath[filePath] = diff
        touch(filePath)
        loadingPaths.remove(filePath)
        tasks.removeValue(forKey: filePath)
        enforceCap(pinnedPaths: pinnedPaths)
    }

    func storeError(_ message: String, for filePath: String) {
        errorsByPath[filePath] = message
        loadingPaths.remove(filePath)
        tasks.removeValue(forKey: filePath)
    }

    func touch(_ filePath: String) {
        accessOrder.removeAll { $0 == filePath }
        accessOrder.append(filePath)
    }

    func evict(_ filePath: String) {
        diffsByPath.removeValue(forKey: filePath)
        errorsByPath.removeValue(forKey: filePath)
        tasks[filePath]?.cancel()
        tasks.removeValue(forKey: filePath)
        loadingPaths.remove(filePath)
        accessOrder.removeAll { $0 == filePath }
    }

    func cancelLoad(for filePath: String) {
        tasks[filePath]?.cancel()
        tasks.removeValue(forKey: filePath)
        loadingPaths.remove(filePath)
    }

    func registerTask(_ task: Task<Void, Never>, for filePath: String) {
        tasks[filePath]?.cancel()
        tasks[filePath] = task
    }

    func collapseAll() {
        tasks.values.forEach { $0.cancel() }
        tasks.removeAll()
        loadingPaths.removeAll()
        errorsByPath.removeAll()
    }

    func clearAll() {
        tasks.values.forEach { $0.cancel() }
        tasks.removeAll()
        diffsByPath.removeAll()
        loadingPaths.removeAll()
        errorsByPath.removeAll()
        accessOrder.removeAll()
    }

    nonisolated func cancelAll() {
        tasks.values.forEach { $0.cancel() }
    }

    private func enforceCap(pinnedPaths: Set<String>) {
        while accessOrder.count > cap {
            let oldest = accessOrder.removeFirst()
            if pinnedPaths.contains(oldest) { continue }
            diffsByPath.removeValue(forKey: oldest)
            errorsByPath.removeValue(forKey: oldest)
        }
    }
}
