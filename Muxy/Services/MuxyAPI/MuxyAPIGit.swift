import Foundation

extension MuxyAPI {
    @MainActor
    enum Git {
        struct Context {
            let extensionID: String
            let appState: AppState
            let projectStore: ProjectStore
            let worktreeStore: WorktreeStore
            let projectGroupStore: ProjectGroupStore
        }

        static let maxLogCount = 1000
        static let maxPRListLimit = 200
        static let maxDiffLineLimit = 100_000

        static func status(
            projectIdentifier: String?,
            local: Bool,
            fresh: Bool,
            context: Context
        ) async -> Result<GitStatusSnapshot, APIError> {
            await cachedRead(projectIdentifier, context, endpoint: "status", params: "local=\(local)", fresh: fresh) { repoPath, git in
                try await GitStatusAggregator.snapshot(
                    repoPath: repoPath,
                    includePullRequest: !local,
                    forceFreshPullRequest: fresh,
                    git: git
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
            ) { repoPath, git in
                try await git.patchAndCompare(
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
            ) { repoPath, git in
                try await git.rawDiff(
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
            ) { repoPath, git in
                try await git.commitLog(
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
            await read(projectIdentifier, context) { repoPath, git in
                try await git.listBranches(repoPath: repoPath)
            }
        }

        static func currentBranch(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<String, APIError> {
            await read(projectIdentifier, context) { repoPath, git in
                try await git.currentBranch(repoPath: repoPath)
            }
        }

        static func aheadBehind(
            projectIdentifier: String?,
            fresh: Bool,
            context: Context
        ) async -> Result<GitRepositoryService.AheadBehind, APIError> {
            await cachedRead(projectIdentifier, context, endpoint: "aheadBehind", fresh: fresh) { repoPath, git in
                let branch = try await git.currentBranch(repoPath: repoPath)
                return await git.aheadBehind(repoPath: repoPath, branch: branch)
            }
        }

        static func repoInfo(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<GitRepositoryService.RepoInfo, APIError> {
            await read(projectIdentifier, context) { repoPath, git in
                try await git.repoInfo(repoPath: repoPath)
            }
        }

        static func pullRequestInfo(
            projectIdentifier: String?,
            fresh: Bool,
            context: Context
        ) async -> Result<GitRepositoryService.PRInfo?, APIError> {
            await cachedRead(projectIdentifier, context, endpoint: "pr.info", fresh: fresh) { repoPath, git in
                let branch = try await git.currentBranch(repoPath: repoPath)
                let headSha = await git.headSha(repoPath: repoPath) ?? branch
                let result = await git.cachedPullRequestInfo(
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
            await cachedRead(projectIdentifier, context, endpoint: "pr.number", fresh: fresh) { repoPath, git in
                let branch = try await git.currentBranch(repoPath: repoPath)
                return await git.pullRequestNumber(repoPath: repoPath, branch: branch)
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
            ) { repoPath, git in
                let remote = await git.githubRemoteName(repoPath: repoPath) ?? "origin"
                return try await git.pullRequestDiff(
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
            includeChecks: Bool,
            context: Context
        ) async -> Result<[GitRepositoryService.PRListItem], APIError> {
            await read(projectIdentifier, context) { repoPath, git in
                try await git.listPullRequests(
                    repoPath: repoPath,
                    filter: filter,
                    limit: min(max(limit, 1), maxPRListLimit),
                    includeChecks: includeChecks
                )
            }
        }

        static func worktrees(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<[GitWorktreeRecord], APIError> {
            await read(projectIdentifier, context) { repoPath, git in
                try await GitWorktreeService.shared.listWorktrees(
                    repoPath: repoPath,
                    context: git.context
                )
            }
        }

        static func stage(
            projectIdentifier: String?,
            paths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "stage", context: context) { repoPath, git in
                if paths.isEmpty {
                    try await git.stageAll(repoPath: repoPath)
                } else {
                    try await git.stageFiles(repoPath: repoPath, paths: paths)
                }
            }
        }

        static func unstage(
            projectIdentifier: String?,
            paths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "unstage", context: context) { repoPath, git in
                if paths.isEmpty {
                    try await git.unstageAll(repoPath: repoPath)
                } else {
                    try await git.unstageFiles(repoPath: repoPath, paths: paths)
                }
            }
        }

        static func discard(
            projectIdentifier: String?,
            paths: [String],
            untrackedPaths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "discard", context: context) { repoPath, git in
                try await git.discardFiles(repoPath: repoPath, paths: paths, untrackedPaths: untrackedPaths)
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
            return await write(projectIdentifier, operation: "commit", context: context) { repoPath, git in
                if stageAll {
                    try await git.stageAll(repoPath: repoPath)
                }
                return try await git.commit(repoPath: repoPath, message: trimmed)
            }
        }

        static func push(
            projectIdentifier: String?,
            setUpstream: Bool,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "push", context: context) { repoPath, git in
                if setUpstream {
                    let branch = try await git.currentBranch(repoPath: repoPath)
                    try await git.pushSetUpstream(repoPath: repoPath, branch: branch)
                    return
                }
                do {
                    try await git.push(repoPath: repoPath)
                } catch GitRepositoryService.GitError.noUpstreamBranch {
                    let branch = try await git.currentBranch(repoPath: repoPath)
                    try await git.pushSetUpstream(repoPath: repoPath, branch: branch)
                }
            }
        }

        static func initRepository(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "init", context: context) { repoPath, git in
                try await git.initRepository(repoPath: repoPath)
            }
        }

        static func pull(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "pull", context: context) { repoPath, git in
                try await git.pull(repoPath: repoPath)
            }
        }

        static func createBranch(
            projectIdentifier: String?,
            name: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("branch name is required")) }
            return await write(projectIdentifier, operation: "branch.create", context: context) { repoPath, git in
                try await git.createAndSwitchBranch(repoPath: repoPath, name: trimmed)
            }
        }

        static func switchBranch(
            projectIdentifier: String?,
            branch: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = branch.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("branch is required")) }
            return await write(projectIdentifier, operation: "branch.switch", context: context) { repoPath, git in
                try await git.switchBranch(repoPath: repoPath, branch: trimmed)
            }
        }

        static func remoteBranches(
            projectIdentifier: String?,
            context: Context
        ) async -> Result<[String], APIError> {
            await read(projectIdentifier, context) { repoPath, git in
                try await git.listRemoteBranches(repoPath: repoPath)
            }
        }

        static func deleteRemoteBranch(
            projectIdentifier: String?,
            branch: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = branch.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("branch is required")) }
            return await write(projectIdentifier, operation: "branch.deleteRemote", context: context) { repoPath, git in
                try await git.deleteRemoteBranch(repoPath: repoPath, branch: trimmed)
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
            return await write(projectIdentifier, operation: "branch.delete", context: context) { repoPath, git in
                try await git.deleteLocalBranch(repoPath: repoPath, branch: trimmed, force: force)
            }
        }

        static func checkout(
            projectIdentifier: String?,
            hash: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = hash.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("hash is required")) }
            return await write(projectIdentifier, operation: "checkout", context: context) { repoPath, git in
                try await git.checkoutDetached(repoPath: repoPath, hash: trimmed)
            }
        }

        static func cherryPick(
            projectIdentifier: String?,
            hash: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = hash.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("hash is required")) }
            return await write(projectIdentifier, operation: "cherryPick", context: context) { repoPath, git in
                try await git.cherryPick(repoPath: repoPath, hash: trimmed)
            }
        }

        static func revert(
            projectIdentifier: String?,
            hash: String,
            context: Context
        ) async -> Result<Void, APIError> {
            let trimmed = hash.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return .failure(.invalidArguments("hash is required")) }
            return await write(projectIdentifier, operation: "revert", context: context) { repoPath, git in
                try await git.revert(repoPath: repoPath, hash: trimmed)
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
            return await write(projectIdentifier, operation: "tag.create", context: context) { repoPath, git in
                try await git.createTag(repoPath: repoPath, name: trimmedName, hash: trimmedHash)
            }
        }

        static func checkoutPullRequest(
            projectIdentifier: String?,
            number: Int,
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "pr.checkout", context: context) { repoPath, git in
                try await git.checkoutPullRequest(repoPath: repoPath, number: number)
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
            let resolvedPath = workspaceContext(projectIdentifier, context: context).isRemote
                ? trimmedPath
                : NSString(string: trimmedPath).expandingTildeInPath
            return await write(projectIdentifier, operation: "pr.checkoutWorktree", context: context) { repoPath, git in
                try await git.createPullRequestWorktree(
                    repoPath: repoPath,
                    path: resolvedPath,
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
            return await write(request.projectIdentifier, operation: "pr.create", context: context) { repoPath, git in
                let branch = try await git.currentBranch(repoPath: repoPath)
                let hasRemote = await git.hasRemoteBranch(repoPath: repoPath, branch: branch)
                if !hasRemote {
                    try await git.pushSetUpstream(repoPath: repoPath, branch: branch)
                }
                let base = request.baseBranch?.trimmingCharacters(in: .whitespacesAndNewlines)
                let resolvedBase: String = if let base, !base.isEmpty {
                    base
                } else {
                    await git.defaultBranch(repoPath: repoPath) ?? "main"
                }
                return try await git.createPullRequest(
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
            await write(projectIdentifier, operation: "pr.merge", context: context) { repoPath, git in
                try await git.mergePullRequest(
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
            await write(projectIdentifier, operation: "pr.close", context: context) { repoPath, git in
                try await git.closePullRequest(repoPath: repoPath, number: number)
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
        ) async -> Result<String, APIError> {
            let trimmedPath = request.path.trimmingCharacters(in: .whitespacesAndNewlines)
            let trimmedBranch = request.branch.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedPath.isEmpty, !trimmedBranch.isEmpty else {
                return .failure(.invalidArguments("path and branch are required"))
            }
            let workspaceContext = workspaceContext(request.projectIdentifier, context: context)
            let worktreePath = workspaceContext.isRemote ? trimmedPath : NSString(string: trimmedPath).expandingTildeInPath
            return await write(request.projectIdentifier, operation: "worktree.add", context: context) { repoPath, _ in
                let base = request.baseBranch?.trimmingCharacters(in: .whitespacesAndNewlines)
                try await GitWorktreeService.shared.addWorktree(
                    repoPath: repoPath,
                    path: worktreePath,
                    branch: trimmedBranch,
                    createBranch: request.createBranch,
                    baseBranch: request.createBranch && base?.isEmpty == false ? base : nil,
                    context: workspaceContext
                )
                return worktreePath
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
            let workspaceContext = workspaceContext(projectIdentifier, context: context)
            let expandedPath = workspaceContext.isRemote ? trimmedPath : NSString(string: trimmedPath).expandingTildeInPath

            guard let tracked = trackedWorktree(
                path: expandedPath,
                context: context,
                workspaceContext: workspaceContext
            )
            else {
                return await write(projectIdentifier, operation: "worktree.remove", context: context) { repoPath, _ in
                    try await GitWorktreeService.shared.removeWorktree(
                        repoPath: repoPath,
                        path: expandedPath,
                        force: force,
                        context: workspaceContext
                    )
                }
            }

            if !force, await GitWorktreeService.shared.hasUncommittedChanges(
                worktreePath: tracked.worktree.path,
                context: workspaceContext
            ) {
                return .failure(.invalidArguments("worktree has uncommitted changes; pass force to remove it"))
            }

            let result = await write(projectIdentifier, operation: "worktree.remove", context: context) { _, _ in
                try await WorktreeStore.cleanupOnDisk(
                    worktree: tracked.worktree,
                    repoPath: tracked.project.path,
                    context: workspaceContext
                )
            }
            if case .success = result {
                forgetWorktree(project: tracked.project, worktree: tracked.worktree, context: context)
            }
            return result
        }

        static func trackedWorktree(
            path: String,
            context: Context,
            workspaceContext: WorkspaceContext = .local
        ) -> (project: Project, worktree: Worktree)? {
            let target = GitWorktreeService.canonicalPath(path, context: workspaceContext)
            for project in context.projectStore.projects {
                guard let worktree = context.worktreeStore.list(for: project.id).first(where: {
                    GitWorktreeService.canonicalPath($0.path, context: workspaceContext) == target
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
            _ work: (String, GitRepositoryService) async throws -> T
        ) async -> Result<T, APIError> {
            guard let resolved = resolveRepo(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            do {
                return try await .success(work(resolved.path, resolved.git))
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
            _ work: (String, GitRepositoryService) async throws -> T
        ) async -> Result<T, APIError> {
            guard let resolved = resolveRepo(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            let key = GitMetadataCache.ReadKey(repoPath: resolved.path, endpoint: endpoint, params: params)
            let signature = await resolved.git.repoSignature(repoPath: resolved.path)
            if !fresh, let cached: T = GitMetadataCache.shared.cachedRead(key, signature: signature) {
                return .success(cached)
            }
            do {
                let value = try await work(resolved.path, resolved.git)
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
            _ work: (String, GitRepositoryService) async throws -> T
        ) async -> Result<T, APIError> {
            guard let resolved = resolveRepo(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            let consent = ExtensionConsentRequestBuilder.make(
                extensionID: context.extensionID,
                verb: .gitWrite,
                payload: .git(operation: operation, repoPath: resolved.path),
                source: "muxy-api"
            )
            guard await ExtensionConsentService.shared.gate(consent) == .allow else {
                return .failure(.consentDenied(verb: "git.\(operation)"))
            }
            do {
                let value = try await work(resolved.path, resolved.git)
                GitMetadataCache.shared.invalidateReads(repoPath: resolved.path)
                return .success(value)
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }

        private static func workspaceContext(_ projectIdentifier: String?, context: Context) -> WorkspaceContext {
            guard let project = context.projectGroupStore.resolveProject(
                identifier: projectIdentifier,
                localProjects: context.projectStore.projects,
                activeProjectID: context.appState.activeProjectID
            )
            else { return .local }
            return context.projectGroupStore.workspaceContext(for: project)
        }

        private static func resolveRepo(
            _ projectIdentifier: String?,
            context: Context
        ) -> (path: String, git: GitRepositoryService)? {
            guard let project = context.projectGroupStore.resolveProject(
                identifier: projectIdentifier,
                localProjects: context.projectStore.projects,
                activeProjectID: context.appState.activeProjectID
            )
            else { return nil }
            let git = GitRepositoryService(context: context.projectGroupStore.workspaceContext(for: project))
            if let worktreeID = context.appState.activeWorktreeID[project.id],
               let worktree = context.worktreeStore.worktree(projectID: project.id, worktreeID: worktreeID)
            {
                return (worktree.path, git)
            }
            return (project.path, git)
        }
    }
}
