import Foundation

extension MuxyAPI {
    @MainActor
    enum Git {
        struct Context {
            let extensionID: String
            let appState: AppState
            let projectStore: ProjectStore
            let worktreeStore: WorktreeStore
        }

        private static let service = GitRepositoryService()

        static let maxLogCount = 1000
        static let maxPRListLimit = 200
        static let maxDiffLineLimit = 100_000

        static func status(
            projectIdentifier: String?,
            local: Bool,
            fresh: Bool,
            context: Context
        ) async -> Result<GitStatusSnapshot, APIError> {
            await cachedRead(projectIdentifier, context, endpoint: "status", params: "local=\(local)", fresh: fresh) { repoPath in
                try await GitStatusAggregator.snapshot(
                    repoPath: repoPath,
                    includePullRequest: !local,
                    forceFreshPullRequest: fresh,
                    git: service
                )
            }
        }

        struct DiffRequest {
            let projectIdentifier: String?
            let filePath: String?
            let staged: Bool?
            let lineLimit: Int?
            let fresh: Bool
        }

        static func diff(
            _ request: DiffRequest,
            context: Context
        ) async -> Result<GitRepositoryService.PatchAndCompareResult, APIError> {
            guard let filePath = request.filePath, !filePath.isEmpty else {
                return .failure(.invalidArguments("filePath is required"))
            }
            let limit = request.lineLimit
            let params = "file=\(filePath);staged=\(request.staged.map(String.init) ?? "nil");limit=\(limit.map(String.init) ?? "nil")"
            return await cachedRead(
                request.projectIdentifier,
                context,
                endpoint: "diff",
                params: params,
                fresh: request.fresh
            ) { repoPath in
                try await service.patchAndCompare(
                    repoPath: repoPath,
                    filePath: filePath,
                    lineLimit: limit.map { min($0, maxDiffLineLimit) },
                    hints: diffHints(staged: request.staged)
                )
            }
        }

        static func rawDiff(
            _ request: DiffRequest,
            context: Context
        ) async -> Result<GitRepositoryService.RawDiffResult, APIError> {
            let staged = request.staged ?? false
            let limit = request.lineLimit
            let params = "file=\(request.filePath ?? "nil");staged=\(staged);limit=\(limit.map(String.init) ?? "nil")"
            return await cachedRead(
                request.projectIdentifier,
                context,
                endpoint: "rawDiff",
                params: params,
                fresh: request.fresh
            ) { repoPath in
                try await service.rawDiff(
                    repoPath: repoPath,
                    filePath: request.filePath,
                    range: nil,
                    staged: staged,
                    lineLimit: limit.map { min($0, maxDiffLineLimit) }
                )
            }
        }

        static func log(
            projectIdentifier: String?,
            maxCount: Int,
            skip: Int,
            fresh: Bool,
            context: Context
        ) async -> Result<[GitCommit], APIError> {
            await cachedRead(
                projectIdentifier,
                context,
                endpoint: "log",
                params: "max=\(maxCount);skip=\(skip)",
                fresh: fresh
            ) { repoPath in
                try await service.commitLog(
                    repoPath: repoPath,
                    maxCount: min(max(maxCount, 0), maxLogCount),
                    skip: max(skip, 0)
                )
            }
        }

        static func branches(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<[String], APIError> {
            await read(projectIdentifier, context) { repoPath in
                try await service.listBranches(repoPath: repoPath)
            }
        }

        static func currentBranch(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<String, APIError> {
            await read(projectIdentifier, context) { repoPath in
                try await service.currentBranch(repoPath: repoPath)
            }
        }

        static func aheadBehind(
            projectIdentifier: String?,
            fresh: Bool,
            context: Context
        ) async -> Result<GitRepositoryService.AheadBehind, APIError> {
            await cachedRead(projectIdentifier, context, endpoint: "aheadBehind", fresh: fresh) { repoPath in
                let branch = try await service.currentBranch(repoPath: repoPath)
                return await service.aheadBehind(repoPath: repoPath, branch: branch)
            }
        }

        static func repoInfo(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<GitRepositoryService.RepoInfo, APIError> {
            await read(projectIdentifier, context) { repoPath in
                try await service.repoInfo(repoPath: repoPath)
            }
        }

        static func pullRequestInfo(
            projectIdentifier: String?,
            fresh: Bool,
            context: Context
        ) async -> Result<GitRepositoryService.PRInfo?, APIError> {
            await cachedRead(projectIdentifier, context, endpoint: "pr.info", fresh: fresh) { repoPath in
                let branch = try await service.currentBranch(repoPath: repoPath)
                let headSha = await service.headSha(repoPath: repoPath) ?? branch
                let result = await service.cachedPullRequestInfo(
                    repoPath: repoPath,
                    branch: branch,
                    headSha: headSha,
                    forceFresh: fresh
                )
                guard case let .found(info) = result else { return nil }
                return info
            }
        }

        static func pullRequestNumber(
            projectIdentifier: String?,
            fresh: Bool,
            context: Context
        ) async -> Result<Int?, APIError> {
            await cachedRead(projectIdentifier, context, endpoint: "pr.number", fresh: fresh) { repoPath in
                let branch = try await service.currentBranch(repoPath: repoPath)
                return await service.pullRequestNumber(repoPath: repoPath, branch: branch)
            }
        }

        static func pullRequestDiff(
            projectIdentifier: String?,
            number: Int,
            lineLimit: Int?,
            fresh: Bool,
            context: Context
        ) async -> Result<GitRepositoryService.RawDiffResult, APIError> {
            guard number > 0 else { return .failure(.invalidArguments("number is required")) }
            return await cachedRead(
                projectIdentifier, context, endpoint: "pr.diff",
                params: "n=\(number);limit=\(lineLimit.map(String.init) ?? "nil")", fresh: fresh
            ) { repoPath in
                let remote = await service.githubRemoteName(repoPath: repoPath) ?? "origin"
                return try await service.pullRequestDiff(
                    repoPath: repoPath,
                    number: number,
                    remote: remote,
                    lineLimit: lineLimit.map { min($0, maxDiffLineLimit) }
                )
            }
        }

        static func pullRequestList(
            projectIdentifier: String?,
            filter: GitRepositoryService.PRListFilter,
            limit: Int,
            context: Context
        ) async -> Result<[GitRepositoryService.PRListItem], APIError> {
            await read(projectIdentifier, context) { repoPath in
                try await service.listPullRequests(
                    repoPath: repoPath,
                    filter: filter,
                    limit: min(max(limit, 1), maxPRListLimit)
                )
            }
        }

        static func worktrees(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<[GitWorktreeRecord], APIError> {
            await read(projectIdentifier, context) { repoPath in
                try await GitWorktreeService.shared.listWorktrees(repoPath: repoPath)
            }
        }

        static func stage(
            projectIdentifier: String?,
            paths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "stage", context: context) { repoPath in
                if paths.isEmpty {
                    try await service.stageAll(repoPath: repoPath)
                } else {
                    try await service.stageFiles(repoPath: repoPath, paths: paths)
                }
            }
        }

        static func unstage(
            projectIdentifier: String?,
            paths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "unstage", context: context) { repoPath in
                if paths.isEmpty {
                    try await service.unstageAll(repoPath: repoPath)
                } else {
                    try await service.unstageFiles(repoPath: repoPath, paths: paths)
                }
            }
        }

        static func discard(
            projectIdentifier: String?,
            paths: [String],
            untrackedPaths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "discard", context: context) { repoPath in
                try await service.discardFiles(repoPath: repoPath, paths: paths, untrackedPaths: untrackedPaths)
            }
        }

        static func commit(
            projectIdentifier: String?,
            message: String,
            stageAll: Bool,
            context: Context
        ) async -> Result<String, APIError> {
            let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("commit message is required")) }
            return await write(projectIdentifier, operation: "commit", context: context) { repoPath in
                if stageAll {
                    try await service.stageAll(repoPath: repoPath)
                }
                return try await service.commit(repoPath: repoPath, message: trimmed)
            }
        }

        static func push(
            projectIdentifier: String?,
            setUpstream: Bool,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "push", context: context) { repoPath in
                if setUpstream {
                    let branch = try await service.currentBranch(repoPath: repoPath)
                    try await service.pushSetUpstream(repoPath: repoPath, branch: branch)
                    return
                }
                do {
                    try await service.push(repoPath: repoPath)
                } catch GitRepositoryService.GitError.noUpstreamBranch {
                    let branch = try await service.currentBranch(repoPath: repoPath)
                    try await service.pushSetUpstream(repoPath: repoPath, branch: branch)
                }
            }
        }

        static func initRepository(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "init", context: context) { repoPath in
                try await service.initRepository(repoPath: repoPath)
            }
        }

        static func pull(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "pull", context: context) { repoPath in
                try await service.pull(repoPath: repoPath)
            }
        }

        static func createBranch(
            projectIdentifier: String?,
            name: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("branch name is required")) }
            return await write(projectIdentifier, operation: "branch.create", context: context) { repoPath in
                try await service.createAndSwitchBranch(repoPath: repoPath, name: trimmed)
            }
        }

        static func switchBranch(
            projectIdentifier: String?,
            branch: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = branch.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("branch is required")) }
            return await write(projectIdentifier, operation: "branch.switch", context: context) { repoPath in
                try await service.switchBranch(repoPath: repoPath, branch: trimmed)
            }
        }

        static func remoteBranches(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<[String], APIError> {
            await read(projectIdentifier, context) { repoPath in
                try await service.listRemoteBranches(repoPath: repoPath)
            }
        }

        static func deleteRemoteBranch(
            projectIdentifier: String?,
            branch: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = branch.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("branch is required")) }
            return await write(projectIdentifier, operation: "branch.deleteRemote", context: context) { repoPath in
                try await service.deleteRemoteBranch(repoPath: repoPath, branch: trimmed)
            }
        }

        static func deleteLocalBranch(
            projectIdentifier: String?,
            name: String,
            force: Bool,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("name is required")) }
            return await write(projectIdentifier, operation: "branch.delete", context: context) { repoPath in
                try await service.deleteLocalBranch(repoPath: repoPath, branch: trimmed, force: force)
            }
        }

        static func checkout(
            projectIdentifier: String?,
            hash: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = hash.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("hash is required")) }
            return await write(projectIdentifier, operation: "checkout", context: context) { repoPath in
                try await service.checkoutDetached(repoPath: repoPath, hash: trimmed)
            }
        }

        static func cherryPick(
            projectIdentifier: String?,
            hash: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = hash.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("hash is required")) }
            return await write(projectIdentifier, operation: "cherryPick", context: context) { repoPath in
                try await service.cherryPick(repoPath: repoPath, hash: trimmed)
            }
        }

        static func revert(
            projectIdentifier: String?,
            hash: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = hash.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("hash is required")) }
            return await write(projectIdentifier, operation: "revert", context: context) { repoPath in
                try await service.revert(repoPath: repoPath, hash: trimmed)
            }
        }

        static func createTag(
            projectIdentifier: String?,
            name: String,
            hash: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
            let trimmedHash = hash.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedName.isEmpty, !trimmedHash.isEmpty else {
                return .failure(.invalidArguments("name and hash are required"))
            }
            return await write(projectIdentifier, operation: "tag.create", context: context) { repoPath in
                try await service.createTag(repoPath: repoPath, name: trimmedName, hash: trimmedHash)
            }
        }

        static func checkoutPullRequest(
            projectIdentifier: String?,
            number: Int,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "pr.checkout", context: context) { repoPath in
                try await service.checkoutPullRequest(repoPath: repoPath, number: number)
            }
        }

        static func checkoutPullRequestWorktree(
            projectIdentifier: String?,
            path: String,
            number: Int,
            context: Context
        ) async -> Result<String, APIError> {
            let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedPath.isEmpty else { return .failure(.invalidArguments("path is required")) }
            return await write(projectIdentifier, operation: "pr.checkoutWorktree", context: context) { repoPath in
                try await service.createPullRequestWorktree(
                    repoPath: repoPath,
                    path: NSString(string: trimmedPath).expandingTildeInPath,
                    number: number
                )
            }
        }

        struct CreatePRRequest {
            let projectIdentifier: String?
            let title: String
            let body: String
            let baseBranch: String?
            let draft: Bool
        }

        static func createPullRequest(
            _ request: CreatePRRequest,
            context: Context
        ) async -> Result<GitRepositoryService.PRInfo, APIError> {
            let trimmedTitle = request.title.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedTitle.isEmpty else { return .failure(.invalidArguments("PR title is required")) }
            return await write(request.projectIdentifier, operation: "pr.create", context: context) { repoPath in
                let branch = try await service.currentBranch(repoPath: repoPath)
                let hasRemote = await service.hasRemoteBranch(repoPath: repoPath, branch: branch)
                if !hasRemote {
                    try await service.pushSetUpstream(repoPath: repoPath, branch: branch)
                }
                let base = request.baseBranch?.trimmingCharacters(in: .whitespacesAndNewlines)
                let resolvedBase: String = if let base, !base.isEmpty {
                    base
                } else {
                    await service.defaultBranch(repoPath: repoPath) ?? "main"
                }
                return try await service.createPullRequest(
                    repoPath: repoPath,
                    branch: branch,
                    baseBranch: resolvedBase,
                    title: trimmedTitle,
                    body: request.body,
                    draft: request.draft
                )
            }
        }

        static func mergePullRequest(
            projectIdentifier: String?,
            number: Int,
            method: GitRepositoryService.PRMergeMethod,
            deleteBranch: Bool,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "pr.merge", context: context) { repoPath in
                try await service.mergePullRequest(
                    repoPath: repoPath,
                    number: number,
                    method: method,
                    deleteBranch: deleteBranch
                )
            }
        }

        static func closePullRequest(
            projectIdentifier: String?,
            number: Int,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "pr.close", context: context) { repoPath in
                try await service.closePullRequest(repoPath: repoPath, number: number)
            }
        }

        struct AddWorktreeRequest {
            let projectIdentifier: String?
            let path: String
            let branch: String
            let createBranch: Bool
            let baseBranch: String?
        }

        static func addWorktree(
            _ request: AddWorktreeRequest,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmedPath = request.path.trimmingCharacters(in: .whitespacesAndNewlines)
            let trimmedBranch = request.branch.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedPath.isEmpty, !trimmedBranch.isEmpty else {
                return .failure(.invalidArguments("path and branch are required"))
            }
            return await write(request.projectIdentifier, operation: "worktree.add", context: context) { repoPath in
                let base = request.baseBranch?.trimmingCharacters(in: .whitespacesAndNewlines)
                try await GitWorktreeService.shared.addWorktree(
                    repoPath: repoPath,
                    path: NSString(string: trimmedPath).expandingTildeInPath,
                    branch: trimmedBranch,
                    createBranch: request.createBranch,
                    baseBranch: request.createBranch && base?.isEmpty == false ? base : nil
                )
            }
        }

        static func removeWorktree(
            projectIdentifier: String?,
            path: String,
            force: Bool,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedPath.isEmpty else { return .failure(.invalidArguments("path is required")) }
            let expandedPath = NSString(string: trimmedPath).expandingTildeInPath

            guard let tracked = trackedWorktree(path: expandedPath, context: context) else {
                return await write(projectIdentifier, operation: "worktree.remove", context: context) { repoPath in
                    try await GitWorktreeService.shared.removeWorktree(
                        repoPath: repoPath,
                        path: expandedPath,
                        force: force
                    )
                }
            }

            if !force, await GitWorktreeService.shared.hasUncommittedChanges(worktreePath: tracked.worktree.path) {
                return .failure(.invalidArguments("worktree has uncommitted changes; pass force to remove it"))
            }

            let result = await write(projectIdentifier, operation: "worktree.remove", context: context) { _ in
                try await WorktreeStore.cleanupOnDisk(worktree: tracked.worktree, repoPath: tracked.project.path)
            }
            if case .success = result {
                forgetWorktree(project: tracked.project, worktree: tracked.worktree, context: context)
            }
            return result
        }

        static func trackedWorktree(
            path: String,
            context: Context
        ) -> (project: Project, worktree: Worktree)? {
            let target = GitWorktreeService.canonicalPath(path)
            for project in context.projectStore.projects {
                guard let worktree = context.worktreeStore.list(for: project.id).first(where: {
                    GitWorktreeService.canonicalPath($0.path) == target
                }), worktree.canBeRemoved
                else { continue }
                return (project, worktree)
            }
            return nil
        }

        static func forgetWorktree(project: Project, worktree: Worktree, context: Context) {
            let remaining = context.worktreeStore.list(for: project.id).filter { $0.id != worktree.id }
            let replacement = remaining.first { $0.id == context.appState.activeWorktreeID[project.id] }
                ?? remaining.first { $0.isPrimary }
                ?? remaining.first
            context.appState.removeWorktree(projectID: project.id, worktree: worktree, replacement: replacement)
            context.worktreeStore.remove(worktreeID: worktree.id, from: project.id)
        }

        private static func diffHints(staged: Bool?) -> GitRepositoryService.DiffHints {
            guard let staged else {
                return GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: false, isUntrackedOrNew: false)
            }
            return GitRepositoryService.DiffHints(hasStaged: staged, hasUnstaged: !staged, isUntrackedOrNew: false)
        }

        private static func read<T: Sendable>(
            _ projectIdentifier: String?,
            _ context: Context,
            _ work: (String) async throws -> T
        ) async -> Result<T, APIError> {
            guard let repoPath = repoPath(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            do {
                return try await .success(work(repoPath))
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }

        private static func cachedRead<T: Sendable>(
            _ projectIdentifier: String?,
            _ context: Context,
            endpoint: String,
            params: String = "",
            fresh: Bool,
            _ work: (String) async throws -> T
        ) async -> Result<T, APIError> {
            guard let repoPath = repoPath(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            let key = GitMetadataCache.ReadKey(repoPath: repoPath, endpoint: endpoint, params: params)
            let signature = await service.repoSignature(repoPath: repoPath)
            if !fresh, let cached: T = GitMetadataCache.shared.cachedRead(key, signature: signature) {
                return .success(cached)
            }
            do {
                let value = try await work(repoPath)
                GitMetadataCache.shared.storeRead(value, key: key, signature: signature)
                return .success(value)
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }

        private static func write<T: Sendable>(
            _ projectIdentifier: String?,
            operation: String,
            context: Context,
            _ work: (String) async throws -> T
        ) async -> Result<T, APIError> {
            guard let repoPath = repoPath(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            let consent = ExtensionConsentRequestBuilder.make(
                extensionID: context.extensionID,
                verb: .gitWrite,
                payload: .git(operation: operation, repoPath: repoPath),
                source: "muxy-api"
            )
            guard await ExtensionConsentService.shared.gate(consent) == .allow else {
                return .failure(.consentDenied(verb: "git.\(operation)"))
            }
            do {
                let value = try await work(repoPath)
                GitMetadataCache.shared.invalidateReads(repoPath: repoPath)
                return .success(value)
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }

        private static func repoPath(_ projectIdentifier: String?, context: Context) -> String? {
            let project: Project? = if let projectIdentifier, !projectIdentifier.isEmpty {
                matchProject(projectIdentifier, in: context.projectStore.projects)
            } else if let activeProjectID = context.appState.activeProjectID {
                context.projectStore.projects.first { $0.id == activeProjectID }
            } else {
                nil
            }
            guard let project else { return nil }
            if let worktreeID = context.appState.activeWorktreeID[project.id],
               let worktree = context.worktreeStore.worktree(projectID: project.id, worktreeID: worktreeID)
            {
                return worktree.path
            }
            return project.path
        }

        private static func matchProject(_ identifier: String, in projects: [Project]) -> Project? {
            let standardizedPath = URL(fileURLWithPath: identifier).standardizedFileURL.path
            return projects.first { project in
                project.id.uuidString == identifier
                    || project.name.localizedCaseInsensitiveCompare(identifier) == .orderedSame
                    || URL(fileURLWithPath: project.path).standardizedFileURL.path == standardizedPath
            }
        }
    }
}
