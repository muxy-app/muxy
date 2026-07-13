import Foundation
import os

private let repositoryAIActionsLogger = Logger(subsystem: "app.muxy", category: "RepositoryAIActions")

struct RepositoryAIPullRequestRequest: Equatable {
    let branch: String
    let baseBranch: String
    let title: String
    let body: String
    let draft: Bool
}

protocol RepositoryAIGitOperating: Sendable {
    func currentBranch(repoPath: String) async throws -> String
    func changedFiles(repoPath: String) async throws -> [GitStatusFile]
    func rawDiff(
        repoPath: String,
        filePath: String?,
        range: GitRepositoryService.DiffRange?,
        staged: Bool,
        lineLimit: Int?
    ) async throws -> GitRepositoryService.RawDiffResult
    func stageAll(repoPath: String) async throws
    func commit(repoPath: String, message: String) async throws -> String
    func push(repoPath: String) async throws
    func pushSetUpstream(repoPath: String, branch: String) async throws
    func defaultBranch(repoPath: String) async -> String?
    func listBranches(repoPath: String) async throws -> [String]
    func listRemoteBranches(repoPath: String) async throws -> [String]
    func createAndSwitchBranch(repoPath: String, name: String) async throws
    func commitLog(repoPath: String, maxCount: Int, skip: Int) async throws -> [GitCommit]
    func createPullRequest(repoPath: String, request: RepositoryAIPullRequestRequest) async throws
        -> GitRepositoryService.PRInfo
}

@MainActor
@Observable
final class RepositoryAIActionsService {
    struct Context: Equatable {
        let repositoryID: String
        let path: String
        let workspaceContext: WorkspaceContext
        let expectedBranch: String
        let hasUpstream: Bool
    }

    enum StartError: LocalizedError, Equatable {
        case noProviderAvailable
        case providerUnavailable(String)
        case cliNotInstalled(String)
        case alreadyRunning

        var errorDescription: String? {
            switch self {
            case .noProviderAvailable:
                "No supported AI provider CLI is available."
            case let .providerUnavailable(providerID):
                "The selected AI provider \"\(providerID)\" is no longer supported."
            case let .cliNotInstalled(providerName):
                "\(providerName) CLI is not installed. Choose another provider or install its CLI."
            case .alreadyRunning:
                "An AI repository action is already running for this worktree."
            }
        }
    }

    enum WorkflowError: LocalizedError, Equatable {
        case noChangesToCommit
        case noChangesForPullRequest
        case contextChanged

        var errorDescription: String? {
            switch self {
            case .noChangesToCommit:
                "There are no changes to commit."
            case .noChangesForPullRequest:
                "There are no changes to include in a pull request."
            case .contextChanged:
                "The active branch changed while the AI metadata was being generated."
            }
        }
    }

    enum Outcome: Equatable {
        case committed(String)
        case pullRequestCreated(String)
    }

    typealias TextGeneration = (
        _ prompt: String,
        _ configuration: AIAgentLaunchConfiguration,
        _ providerName: String,
        _ workingDirectory: String,
        _ context: WorkspaceContext
    ) async throws -> String

    static let shared = RepositoryAIActionsService()

    private(set) var activeRuns: [String: RepositoryAIAction] = [:]

    @ObservationIgnored private var tasks: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private let generateText: TextGeneration

    init(generateText: TextGeneration? = nil) {
        self.generateText = generateText ?? { prompt, configuration, providerName, workingDirectory, context in
            try await RepositoryAITextGenerator().generate(
                prompt: prompt,
                configuration: configuration,
                providerName: providerName,
                workingDirectory: workingDirectory,
                context: context
            )
        }
    }

    static func resolveProvider(
        for action: RepositoryAIAction,
        providers: [any AIAgentLaunchProvider],
        installedProviderIDs: Set<String>,
        isRemote: Bool,
        defaults: UserDefaults = .standard
    ) -> (any AIAgentLaunchProvider)? {
        let configuredID = RepositoryAIActionPreferences.configuredProviderID(for: action, defaults: defaults)
        if !configuredID.isEmpty {
            return providers.first { $0.id == configuredID }
        }
        if isRemote {
            return providers.first
        }
        return providers.first { installedProviderIDs.contains($0.id) }
    }

    func isRunning(repositoryID: String, action: RepositoryAIAction? = nil) -> Bool {
        guard let running = activeRuns[repositoryID] else { return false }
        return action == nil || running == action
    }

    func start(
        action: RepositoryAIAction,
        context: Context,
        providers: [any AIAgentLaunchProvider],
        installedProviderIDs: Set<String>,
        defaults: UserDefaults = .standard
    ) throws {
        guard activeRuns[context.repositoryID] == nil else {
            throw StartError.alreadyRunning
        }
        let configuredID = RepositoryAIActionPreferences.configuredProviderID(for: action, defaults: defaults)
        guard configuredID.isEmpty || providers.contains(where: { $0.id == configuredID }) else {
            throw StartError.providerUnavailable(configuredID)
        }
        guard let provider = Self.resolveProvider(
            for: action,
            providers: providers,
            installedProviderIDs: installedProviderIDs,
            isRemote: context.workspaceContext.isRemote,
            defaults: defaults
        )
        else {
            throw StartError.noProviderAvailable
        }
        guard context.workspaceContext.isRemote || installedProviderIDs.contains(provider.id) else {
            throw StartError.cliNotInstalled(provider.displayName)
        }

        activeRuns[context.repositoryID] = action
        let instructions = RepositoryAIActionPreferences.prompt(for: action, defaults: defaults)
        tasks[context.repositoryID] = Task { [weak self] in
            guard let self else { return }
            let result: Result<Outcome, Error>
            do {
                let git = GitRepositoryService(context: context.workspaceContext)
                let outcome = switch action {
                case .commit:
                    try await performCommit(
                        context: context,
                        provider: provider,
                        instructions: instructions,
                        git: git
                    )
                case .createPullRequest:
                    try await performCreatePullRequest(
                        context: context,
                        provider: provider,
                        instructions: instructions,
                        git: git
                    )
                }
                result = .success(outcome)
            } catch {
                result = .failure(error)
            }
            finish(action: action, context: context, providerName: provider.displayName, result: result)
        }
    }

    func performCommit(
        context: Context,
        provider: any AIAgentLaunchProvider,
        instructions: String,
        git: any RepositoryAIGitOperating
    ) async throws -> Outcome {
        try await git.stageAll(repoPath: context.path)
        let metadataContext = try await makeMetadataContext(
            context: context,
            git: git,
            includesBranchDiff: false
        )
        guard !metadataContext.stagedDiff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw WorkflowError.noChangesToCommit
        }
        try Task.checkCancellation()
        let prompt = try RepositoryAIMetadataPromptBuilder.prompt(
            for: .commit,
            instructions: instructions,
            context: metadataContext
        )
        let response = try await generateText(
            prompt,
            provider.agentLaunchConfiguration,
            provider.displayName,
            context.path,
            context.workspaceContext
        )
        try Task.checkCancellation()
        let metadata = try RepositoryAIResponseDecoder.decode(RepositoryAICommitMetadata.self, from: response)
        let message = try RepositoryAIMetadataValidator.commit(metadata)
        try await verifyBranch(context, git: git)
        let hash = try await git.commit(repoPath: context.path, message: message)
        if context.hasUpstream {
            try await git.push(repoPath: context.path)
        } else {
            try await git.pushSetUpstream(repoPath: context.path, branch: context.expectedBranch)
        }
        return .committed(hash)
    }

    func performCreatePullRequest(
        context: Context,
        provider: any AIAgentLaunchProvider,
        instructions: String,
        git: any RepositoryAIGitOperating
    ) async throws -> Outcome {
        try await git.stageAll(repoPath: context.path)
        let metadataContext = try await makeMetadataContext(
            context: context,
            git: git,
            includesBranchDiff: true
        )
        guard metadataContext.hasPullRequestChanges else {
            throw WorkflowError.noChangesForPullRequest
        }
        try Task.checkCancellation()
        let prompt = try RepositoryAIMetadataPromptBuilder.prompt(
            for: .createPullRequest,
            instructions: instructions,
            context: metadataContext
        )
        let response = try await generateText(
            prompt,
            provider.agentLaunchConfiguration,
            provider.displayName,
            context.path,
            context.workspaceContext
        )
        try Task.checkCancellation()
        let rawMetadata = try RepositoryAIResponseDecoder.decode(RepositoryAIPullRequestMetadata.self, from: response)
        async let localBranchesTask = git.listBranches(repoPath: context.path)
        async let remoteBranchesTask = git.listRemoteBranches(repoPath: context.path)
        let (localBranches, remoteBranches) = try await (localBranchesTask, remoteBranchesTask)
        let metadata = try RepositoryAIMetadataValidator.pullRequest(
            rawMetadata,
            currentBranch: context.expectedBranch,
            localBranches: Set(localBranches),
            remoteBranches: Set(remoteBranches)
        )
        try Task.checkCancellation()
        try await verifyBranch(context, git: git)
        try await git.createAndSwitchBranch(repoPath: context.path, name: metadata.newBranchName)
        if !metadataContext.stagedDiff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            _ = try await git.commit(repoPath: context.path, message: metadata.title)
        }
        try await git.pushSetUpstream(repoPath: context.path, branch: metadata.newBranchName)
        let pullRequest = try await git.createPullRequest(
            repoPath: context.path,
            request: RepositoryAIPullRequestRequest(
                branch: metadata.newBranchName,
                baseBranch: metadata.targetBranchName,
                title: metadata.title,
                body: metadata.summary,
                draft: false
            )
        )
        return .pullRequestCreated(pullRequest.url)
    }

    private func makeMetadataContext(
        context: Context,
        git: any RepositoryAIGitOperating,
        includesBranchDiff: Bool
    ) async throws -> RepositoryAIMetadataContext {
        async let filesTask = git.changedFiles(repoPath: context.path)
        async let commitsTask = git.commitLog(repoPath: context.path, maxCount: 12, skip: 0)
        async let defaultBranchTask = git.defaultBranch(repoPath: context.path)
        async let stagedDiffTask = git.rawDiff(
            repoPath: context.path,
            filePath: nil,
            range: nil,
            staged: true,
            lineLimit: 800
        )

        let defaultBranch = await defaultBranchTask
        let branchDiff: GitRepositoryService.RawDiffResult? = if includesBranchDiff, let defaultBranch {
            try? await git.rawDiff(
                repoPath: context.path,
                filePath: nil,
                range: GitRepositoryService.DiffRange(baseRef: "origin/\(defaultBranch)", headRef: "HEAD"),
                staged: false,
                lineLimit: 800
            )
        } else {
            nil
        }
        let files = try await filesTask
        let commits = try await commitsTask
        let stagedDiff = try await stagedDiffTask
        try Task.checkCancellation()
        let changedFiles = files.prefix(500).map(\.path)
        return RepositoryAIMetadataContext(
            currentBranch: context.expectedBranch,
            defaultBranch: defaultBranch,
            changedFiles: changedFiles,
            recentCommitSubjects: commits.map(\.subject),
            stagedDiff: stagedDiff.diff,
            branchDiff: branchDiff?.diff,
            diffWasTruncated: stagedDiff.truncated
                || branchDiff?.truncated == true
                || changedFiles.count < files.count
        )
    }

    private func verifyBranch(
        _ context: Context,
        git: any RepositoryAIGitOperating
    ) async throws {
        guard try await git.currentBranch(repoPath: context.path) == context.expectedBranch else {
            throw WorkflowError.contextChanged
        }
    }

    private func finish(
        action: RepositoryAIAction,
        context: Context,
        providerName: String,
        result: Result<Outcome, Error>
    ) {
        tasks.removeValue(forKey: context.repositoryID)
        activeRuns.removeValue(forKey: context.repositoryID)
        NotificationCenter.default.post(
            name: .vcsRepoDidChange,
            object: nil,
            userInfo: ["repoPath": context.path]
        )

        switch result {
        case let .success(.committed(hash)):
            repositoryAIActionsLogger.info("Committed and pushed with \(providerName, privacy: .public)")
            let detail = hash.isEmpty ? nil : "Commit \(hash)"
            ToastState.shared.show(title: "Committed and pushed", body: detail)
        case let .success(.pullRequestCreated(url)):
            repositoryAIActionsLogger.info("Created pull request with \(providerName, privacy: .public)")
            ToastState.shared.show(title: "Pull request created", body: url)
        case let .failure(error):
            repositoryAIActionsLogger.error("\(action.rawValue, privacy: .public) failed: \(error.localizedDescription, privacy: .public)")
            ToastState.shared.show(title: "Could not \(action.title.lowercased())", body: error.localizedDescription)
        }
    }
}
