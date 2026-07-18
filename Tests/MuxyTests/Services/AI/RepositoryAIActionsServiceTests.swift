import Foundation
import Testing

@testable import Muxy

@Suite("Repository AI actions")
@MainActor
struct RepositoryAIActionsServiceTests {
    @Test("auto provider chooses the first locally installed CLI")
    func automaticProviderSelection() throws {
        let defaults = makeDefaults()
        let providers = [
            RepositoryActionProvider(id: "first"),
            RepositoryActionProvider(id: "second"),
        ]

        let provider = try #require(RepositoryAIActionsService.resolveProvider(
            for: .commit,
            providers: providers,
            installedProviderIDs: ["second"],
            isRemote: false,
            defaults: defaults
        ))

        #expect(provider.id == "second")
    }

    @Test("explicit provider selection remains action-specific")
    func explicitProviderSelection() throws {
        let defaults = makeDefaults()
        defaults.set("second", forKey: RepositoryAIAction.createPullRequest.providerKey)
        let providers = [
            RepositoryActionProvider(id: "first"),
            RepositoryActionProvider(id: "second"),
        ]

        let provider = try #require(RepositoryAIActionsService.resolveProvider(
            for: .createPullRequest,
            providers: providers,
            installedProviderIDs: ["first", "second"],
            isRemote: false,
            defaults: defaults
        ))

        #expect(provider.id == "second")
    }

    @Test("remote auto selection defers CLI discovery to the remote host")
    func remoteProviderSelection() throws {
        let defaults = makeDefaults()
        let providers = [
            RepositoryActionProvider(id: "first"),
            RepositoryActionProvider(id: "second"),
        ]

        let provider = try #require(RepositoryAIActionsService.resolveProvider(
            for: .commit,
            providers: providers,
            installedProviderIDs: ["second"],
            isRemote: true,
            defaults: defaults
        ))

        #expect(provider.id == "first")
    }

    @Test("local launch uses the exact executable found during detection")
    func localLaunchUsesResolvedExecutable() throws {
        let provider = RepositoryActionProvider(
            id: "codex",
            executablePath: "/custom/bin/codex"
        )

        let configuration = try #require(RepositoryAIActionsService.resolveLaunchConfiguration(
            provider: provider,
            isRemote: false
        ))

        #expect(configuration.executable == "/custom/bin/codex")
    }

    @Test("remote launch keeps the executable name for host-side resolution")
    func remoteLaunchKeepsExecutableName() throws {
        let provider = RepositoryActionProvider(
            id: "codex",
            executablePath: "/custom/bin/codex"
        )

        let configuration = try #require(RepositoryAIActionsService.resolveLaunchConfiguration(
            provider: provider,
            isRemote: true
        ))

        #expect(configuration.executable == "codex")
    }

    @Test("commit stages, generates metadata, commits, and pushes through native Git")
    func commitWorkflow() async throws {
        let git = RepositoryActionGitMock(stagedDiff: "diff --git a/file b/file")
        let recorder = RepositoryActionPromptRecorder(response: #"{"message":"feat: add native workflow"}"#)
        let service = makeService(recorder: recorder)

        let outcome = try await service.performCommit(
            context: makeContext(hasUpstream: true),
            provider: RepositoryActionProvider(id: "claude"),
            instructions: "Use the repository style",
            git: git
        )

        #expect(outcome == .committed("abc123"))
        #expect(await git.recordedOperations() == [
            "stageAll",
            "commit:feat: add native workflow",
            "push",
        ])
        let prompt = try #require(await recorder.recordedPrompt())
        #expect(prompt.contains("Use the repository style"))
        #expect(prompt.contains(#"{"message":"Concise commit subject and optional body"}"#))
    }

    @Test("commit establishes an upstream when the branch has none")
    func commitWorkflowSetsUpstream() async throws {
        let git = RepositoryActionGitMock(stagedDiff: "staged change")
        let recorder = RepositoryActionPromptRecorder(response: #"{"message":"fix: save changes"}"#)
        let service = makeService(recorder: recorder)

        _ = try await service.performCommit(
            context: makeContext(hasUpstream: false),
            provider: RepositoryActionProvider(id: "codex"),
            instructions: "",
            git: git
        )

        #expect(await git.recordedOperations() == [
            "stageAll",
            "commit:fix: save changes",
            "pushSetUpstream:feature/native-actions",
        ])
    }

    @Test("create PR uses AI metadata and native branch, commit, push, and PR operations")
    func pullRequestWorkflow() async throws {
        let git = RepositoryActionGitMock(
            stagedDiff: "working tree diff",
            branchDiff: "committed branch diff",
            localBranches: ["feature/native-actions"],
            remoteBranches: ["main"]
        )
        let response = """
        {
          "title": "Add native repository actions",
          "summary": "Generates metadata with AI and applies it through native services.",
          "newBranchName": "muxy/native-repository-actions",
          "targetBranchName": "main"
        }
        """
        let recorder = RepositoryActionPromptRecorder(response: response)
        let service = makeService(recorder: recorder)

        let outcome = try await service.performCreatePullRequest(
            context: makeContext(),
            provider: RepositoryActionProvider(id: "claude"),
            instructions: "Keep the summary concise",
            git: git
        )

        #expect(outcome == .pullRequestCreated("https://github.com/muxy-app/muxy/pull/42"))
        #expect(await git.recordedOperations() == [
            "stageAll",
            "createAndSwitchBranch:muxy/native-repository-actions",
            "commit:Add native repository actions",
            "pushSetUpstream:muxy/native-repository-actions",
            "createPullRequest:muxy/native-repository-actions:main:Add native repository actions:" +
                "Generates metadata with AI and applies it through native services.",
        ])
    }

    @Test("create PR rejects a clean working tree even when the branch has commits")
    func pullRequestWorkflowRejectsCleanWorkingTreeWithBranchCommits() async throws {
        let git = RepositoryActionGitMock(
            hasChanges: false,
            stagedDiff: "",
            branchDiff: "existing branch commit",
            localBranches: ["feature/native-actions"],
            remoteBranches: ["main"]
        )
        let recorder = RepositoryActionPromptRecorder(response: #"{"title":"Open existing work","summary":"Existing commits.","newBranchName":"muxy/existing-work","targetBranchName":"main"}"#)
        let service = makeService(recorder: recorder)

        await #expect(throws: RepositoryAIActionsService.WorkflowError.noChangesForPullRequest) {
            try await service.performCreatePullRequest(
                context: makeContext(),
                provider: RepositoryActionProvider(id: "claude"),
                instructions: "",
                git: git
            )
        }

        #expect(await git.recordedOperations().isEmpty)
        #expect(await recorder.recordedPrompt() == nil)
    }

    @Test("create PR stops before AI generation when no changes exist")
    func pullRequestWorkflowRejectsEmptyChanges() async throws {
        let git = RepositoryActionGitMock(
            stagedDiff: "",
            branchDiff: "",
            localBranches: ["feature/native-actions"],
            remoteBranches: ["main"]
        )
        let recorder = RepositoryActionPromptRecorder(response: "unused")
        let service = makeService(recorder: recorder)

        await #expect(throws: RepositoryAIActionsService.WorkflowError.noChangesForPullRequest) {
            try await service.performCreatePullRequest(
                context: makeContext(),
                provider: RepositoryActionProvider(id: "claude"),
                instructions: "",
                git: git
            )
        }

        #expect(await git.recordedOperations() == ["stageAll"])
        #expect(await recorder.recordedPrompt() == nil)
    }

    @Test("create PR allows the primary branch when the working tree is dirty")
    func pullRequestWorkflowAllowsDirtyPrimaryBranch() async throws {
        let git = RepositoryActionGitMock(
            currentBranch: "main",
            stagedDiff: "working tree diff",
            localBranches: ["main"],
            remoteBranches: ["main"]
        )
        let recorder = RepositoryActionPromptRecorder(
            response: #"{"title":"Open main changes","summary":"Main changes.","newBranchName":"muxy/main-changes","targetBranchName":"main"}"#
        )
        let service = makeService(recorder: recorder)

        let outcome = try await service.performCreatePullRequest(
            context: makeContext(expectedBranch: "main"),
            provider: RepositoryActionProvider(id: "claude"),
            instructions: "",
            git: git
        )

        #expect(outcome == .pullRequestCreated("https://github.com/muxy-app/muxy/pull/42"))
        #expect(await git.recordedOperations().first == "stageAll")
    }

    @Test("create PR rejects a stale branch context before resolving or staging")
    func pullRequestWorkflowRejectsStaleBranchContext() async throws {
        let git = RepositoryActionGitMock(currentBranch: "main", stagedDiff: "working tree diff")
        let recorder = RepositoryActionPromptRecorder(response: "unused")
        let service = makeService(recorder: recorder)

        await #expect(throws: RepositoryAIActionsService.WorkflowError.contextChanged) {
            try await service.performCreatePullRequest(
                context: makeContext(),
                provider: RepositoryActionProvider(id: "claude"),
                instructions: "",
                git: git
            )
        }

        #expect(await git.recordedOperations().isEmpty)
        #expect(await recorder.recordedPrompt() == nil)
    }

    @Test("create PR rechecks the branch after reading working tree status")
    func pullRequestWorkflowRejectsBranchChangeDuringStatusResolution() async throws {
        let git = RepositoryActionGitMock(
            currentBranch: "feature/native-actions",
            currentBranchAfterStatusResolution: "main",
            stagedDiff: "working tree diff"
        )
        let recorder = RepositoryActionPromptRecorder(response: "unused")
        let service = makeService(recorder: recorder)

        await #expect(throws: RepositoryAIActionsService.WorkflowError.contextChanged) {
            try await service.performCreatePullRequest(
                context: makeContext(),
                provider: RepositoryActionProvider(id: "claude"),
                instructions: "",
                git: git
            )
        }

        #expect(await git.recordedOperations().isEmpty)
        #expect(await recorder.recordedPrompt() == nil)
    }

    @Test("branch changes prevent mutations after metadata generation")
    func contextChangeStopsCommit() async throws {
        let git = RepositoryActionGitMock(
            currentBranch: "other-branch",
            stagedDiff: "staged change"
        )
        let recorder = RepositoryActionPromptRecorder(response: #"{"message":"fix: safe branch"}"#)
        let service = makeService(recorder: recorder)

        await #expect(throws: RepositoryAIActionsService.WorkflowError.contextChanged) {
            try await service.performCommit(
                context: makeContext(),
                provider: RepositoryActionProvider(id: "claude"),
                instructions: "",
                git: git
            )
        }

        #expect(await git.recordedOperations() == ["stageAll"])
    }

    private func makeService(recorder: RepositoryActionPromptRecorder) -> RepositoryAIActionsService {
        RepositoryAIActionsService { prompt, _, _, _, _ in
            try await recorder.generate(prompt: prompt)
        }
    }

    private func makeContext(
        hasUpstream: Bool = true,
        expectedBranch: String = "feature/native-actions"
    ) -> RepositoryAIActionsService.Context {
        RepositoryAIActionsService.Context(
            repositoryID: "repository",
            path: "/tmp/muxy repository",
            workspaceContext: .local,
            expectedBranch: expectedBranch,
            hasUpstream: hasUpstream
        )
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "RepositoryAIActionsServiceTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}

private struct RepositoryActionProvider: AIAgentLaunchProvider {
    let id: String
    let displayName: String
    let iconName = "sparkles"
    let agentLaunchConfiguration: AIAgentLaunchConfiguration
    let executablePath: String?

    init(id: String, displayName: String? = nil, executablePath: String? = nil) {
        self.id = id
        self.displayName = displayName ?? id
        self.executablePath = executablePath
        agentLaunchConfiguration = AIAgentLaunchConfiguration(
            executable: id,
            headlessArguments: ["--print"]
        )
    }

    func agentCLIExecutablePath() -> String? { executablePath }
    func isAgentCLIInstalled() -> Bool { true }
}

private actor RepositoryActionPromptRecorder {
    private let response: String
    private var prompt: String?

    init(response: String) {
        self.response = response
    }

    func generate(prompt: String) throws -> String {
        self.prompt = prompt
        return response
    }

    func recordedPrompt() -> String? {
        prompt
    }
}

private actor RepositoryActionGitMock: RepositoryAIGitOperating {
    private var currentBranchValue: String
    private let hasChanges: Bool
    private let currentBranchAfterStatusResolution: String?
    private let stagedDiff: String
    private let branchDiff: String
    private let defaultBranchValue: String?
    private let localBranches: [String]
    private let remoteBranches: [String]
    private var operations: [String] = []

    init(
        currentBranch: String = "feature/native-actions",
        hasChanges: Bool = true,
        currentBranchAfterStatusResolution: String? = nil,
        stagedDiff: String,
        branchDiff: String = "",
        localBranches: [String] = ["feature/native-actions"],
        remoteBranches: [String] = ["main"],
        defaultBranch: String? = "main"
    ) {
        currentBranchValue = currentBranch
        self.hasChanges = hasChanges
        self.currentBranchAfterStatusResolution = currentBranchAfterStatusResolution
        self.stagedDiff = stagedDiff
        self.branchDiff = branchDiff
        defaultBranchValue = defaultBranch
        self.localBranches = localBranches
        self.remoteBranches = remoteBranches
    }

    func currentBranch(repoPath _: String) async throws -> String {
        currentBranchValue
    }

    func changedFiles(repoPath _: String) async throws -> [GitStatusFile] {
        if let currentBranchAfterStatusResolution {
            currentBranchValue = currentBranchAfterStatusResolution
        }
        guard hasChanges else { return [] }
        return [GitStatusFile(
            path: "file.txt",
            oldPath: nil,
            xStatus: "M",
            yStatus: " ",
            additions: 1,
            deletions: 0,
            isBinary: false
        )]
    }

    func rawDiff(
        repoPath _: String,
        filePath _: String?,
        range: GitRepositoryService.DiffRange?,
        staged _: Bool,
        lineLimit _: Int?
    ) async throws -> GitRepositoryService.RawDiffResult {
        GitRepositoryService.RawDiffResult(
            diff: range == nil ? stagedDiff : branchDiff,
            truncated: false
        )
    }

    func stageAll(repoPath _: String) async throws {
        operations.append("stageAll")
    }

    func commit(repoPath _: String, message: String) async throws -> String {
        operations.append("commit:\(message)")
        return "abc123"
    }

    func push(repoPath _: String) async throws {
        operations.append("push")
    }

    func pushSetUpstream(repoPath _: String, branch: String) async throws {
        operations.append("pushSetUpstream:\(branch)")
    }

    func defaultBranch(repoPath _: String) async -> String? {
        defaultBranchValue
    }

    func listBranches(repoPath _: String) async throws -> [String] {
        localBranches
    }

    func listRemoteBranches(repoPath _: String) async throws -> [String] {
        remoteBranches
    }

    func createAndSwitchBranch(repoPath _: String, name: String) async throws {
        operations.append("createAndSwitchBranch:\(name)")
        currentBranchValue = name
    }

    func commitLog(repoPath _: String, maxCount _: Int, skip _: Int) async throws -> [GitCommit] {
        []
    }

    func createPullRequest(
        repoPath _: String,
        request: RepositoryAIPullRequestRequest
    ) async throws -> GitRepositoryService.PRInfo {
        operations.append(
            "createPullRequest:\(request.branch):\(request.baseBranch):\(request.title):\(request.body)"
        )
        return GitRepositoryService.PRInfo(
            url: "https://github.com/muxy-app/muxy/pull/42",
            number: 42,
            state: .open,
            isDraft: false,
            baseBranch: request.baseBranch,
            mergeable: true,
            mergeStateStatus: .clean,
            checks: GitRepositoryService.PRChecks(
                status: .none,
                passing: 0,
                failing: 0,
                pending: 0,
                total: 0
            ),
            isCrossRepository: false
        )
    }

    func recordedOperations() -> [String] {
        operations
    }

}
