import Foundation

@MainActor
enum DiffLoader {
    static let previewLineLimit = 20000

    struct Request {
        let repoPath: String
        let filePath: String
        let cacheKey: String
        let hints: GitRepositoryService.DiffHints
        let forceFull: Bool
        let pinnedPaths: Set<String>
    }

    static func load(
        _ request: Request,
        cache: DiffCache,
        git: GitRepositoryService = GitRepositoryService()
    ) {
        cache.markLoading(request.cacheKey)
        let lineLimit = request.forceFull ? nil : previewLineLimit
        let task = Task { @MainActor in
            do {
                let result = try await git.patchAndCompare(
                    repoPath: request.repoPath,
                    filePath: request.filePath,
                    lineLimit: lineLimit,
                    hints: request.hints
                )
                guard !Task.isCancelled else { return }
                cache.store(
                    DiffCache.LoadedDiff(
                        rows: result.rows,
                        additions: result.additions,
                        deletions: result.deletions,
                        truncated: result.truncated
                    ),
                    for: request.cacheKey,
                    pinnedPaths: request.pinnedPaths
                )
            } catch {
                guard !Task.isCancelled else { return }
                let message = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
                cache.storeError(message, for: request.cacheKey)
            }
        }
        cache.registerTask(task, for: request.cacheKey)
    }
}
