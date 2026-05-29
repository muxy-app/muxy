import Foundation
import Testing

@testable import Muxy

@Suite("VCSTabState")
@MainActor
struct VCSTabStateTests {
    @Test("file buckets and change flags derive from status entries")
    func fileBucketsAndChangeFlags() {
        let state = makeState()
        state.files = [
            makeFile("Sources/Staged.swift", xStatus: "M", yStatus: " "),
            makeFile("Sources/Unstaged.swift", xStatus: " ", yStatus: "M"),
            makeFile("Sources/Both.swift", xStatus: "A", yStatus: "M"),
            makeFile("Sources/New.swift", xStatus: "?", yStatus: "?"),
        ]

        #expect(state.stagedFiles.map(\.path) == ["Sources/Staged.swift", "Sources/Both.swift"])
        #expect(state.unstagedFiles.map(\.path) == ["Sources/Unstaged.swift", "Sources/Both.swift", "Sources/New.swift"])
        #expect(state.hasStagedChanges)
        #expect(state.hasAnyChanges)
    }

    @Test("default branch comparison requires both branch names")
    func defaultBranchComparison() {
        let state = makeState()

        #expect(!state.isOnDefaultBranch)

        state.branchName = "main"
        #expect(!state.isOnDefaultBranch)

        state.defaultBranch = "main"
        #expect(state.isOnDefaultBranch)

        state.branchName = "feature"
        #expect(!state.isOnDefaultBranch)
    }

    @Test("PR launch state reflects gh branch fetch and existing PR state")
    func prLaunchState() {
        let state = makeState()

        state.isGhInstalled = false
        #expect(state.prLaunchState == .ghMissing)

        state.isGhInstalled = true
        #expect(state.prLaunchState == .hidden)

        state.branchName = "feature"
        state.hasFetchedPullRequestInfo = true
        #expect(state.prLaunchState == .canCreate)

        let info = makePRInfo(number: 7)
        state.pullRequestInfo = info
        #expect(state.prLaunchState == .hasPR(info))

        state.pullRequestInfo = nil
        state.defaultBranch = "feature"
        #expect(state.prLaunchState == .hidden)

        state.files = [makeFile("Changed.swift", xStatus: " ", yStatus: "M")]
        #expect(state.prLaunchState == .canCreate)
    }

    @Test("folder expansion is tracked separately for staged and unstaged lists")
    func folderExpansionScopesByBucket() {
        let state = makeState()

        state.toggleFolderExpanded("Sources", isStaged: true)
        #expect(state.isFolderExpanded("Sources", isStaged: true))
        #expect(!state.isFolderExpanded("Sources", isStaged: false))

        state.toggleFolderExpanded("Sources", isStaged: false)
        #expect(state.isFolderExpanded("Sources", isStaged: false))

        state.toggleFolderExpanded("Sources", isStaged: true)
        #expect(!state.isFolderExpanded("Sources", isStaged: true))
        #expect(state.isFolderExpanded("Sources", isStaged: false))
    }

    @Test("tree rows are derived from staged and unstaged buckets")
    func treeRowsUseMatchingBuckets() {
        let state = makeState()
        let staged = makeFile("Sources/App/Staged.swift", xStatus: "A", yStatus: " ")
        let unstaged = makeFile("Sources/App/Unstaged.swift", xStatus: " ", yStatus: "M")
        state.files = [staged, unstaged]
        state.expandedStagedFolderPaths = ["Sources/App"]
        state.expandedUnstagedFolderPaths = ["Sources/App"]

        #expect(state.stagedTreeRows == [
            .folder(.init(path: "Sources/App", name: "Sources/App", depth: 0, fileCount: 1)),
            .file(staged, depth: 1),
        ])
        #expect(state.unstagedTreeRows == [
            .folder(.init(path: "Sources/App", name: "Sources/App", depth: 0, fileCount: 1)),
            .file(unstaged, depth: 1),
        ])
    }

    @Test("expanded files load missing diffs and collapse cancels them")
    func expandedFilesManageDiffCache() {
        let state = makeState()
        let file = makeFile("Sources/App.swift", xStatus: " ", yStatus: "M")
        state.files = [file]

        state.toggleExpanded(filePath: file.path)

        #expect(state.expandedFilePaths == [file.path])
        #expect(state.diffCache.isLoading(file.path))

        state.toggleExpanded(filePath: file.path)

        #expect(state.expandedFilePaths.isEmpty)
        #expect(!state.diffCache.isLoading(file.path))
        state.diffCache.cancelAll()
    }

    @Test("setExpanded expands and collapses only requested files")
    func setExpanded() {
        let state = makeState()
        let first = makeFile("A.swift", xStatus: " ", yStatus: "M")
        let second = makeFile("B.swift", xStatus: " ", yStatus: "M")
        state.files = [first, second]

        state.setExpanded(files: [first, second], expanded: true)

        #expect(state.expandedFilePaths == ["A.swift", "B.swift"])
        #expect(state.diffCache.isLoading("A.swift"))
        #expect(state.diffCache.isLoading("B.swift"))

        state.setExpanded(files: [first], expanded: false)

        #expect(state.expandedFilePaths == ["B.swift"])
        #expect(!state.diffCache.isLoading("A.swift"))
        #expect(state.diffCache.isLoading("B.swift"))
        state.diffCache.cancelAll()
    }

    @Test("displayed stats prefer loaded diff over status summary")
    func displayedStatsPreferLoadedDiff() {
        let state = makeState()
        let file = makeFile("Binary.dat", xStatus: " ", yStatus: "M", additions: nil, deletions: nil, isBinary: true)

        let fallback = state.displayedStats(for: file)
        #expect(fallback.additions == nil)
        #expect(fallback.deletions == nil)
        #expect(fallback.binary)

        state.diffCache.store(.init(rows: [], additions: 4, deletions: 2, truncated: false), for: file.path, pinnedPaths: [])

        let loaded = state.displayedStats(for: file)
        #expect(loaded.additions == 4)
        #expect(loaded.deletions == 2)
        #expect(!loaded.binary)
        state.diffCache.cancelAll()
    }

    @Test("commit validates message and staged changes before starting git work")
    func commitValidation() {
        let state = makeState()

        state.commitMessage = "   "
        state.commit()
        #expect(state.statusMessage == "Enter a commit message.")
        #expect(state.statusIsError)
        #expect(!state.isCommitting)

        state.statusMessage = nil
        state.commitMessage = "Commit message"
        state.files = [makeFile("Unstaged.swift", xStatus: " ", yStatus: "M")]
        state.commit()
        #expect(state.statusMessage == "No staged changes to commit.")
        #expect(state.statusIsError)
        #expect(!state.isCommitting)
    }

    @Test("commit message generation validates changes and can be cancelled")
    func commitMessageGenerationValidationAndCancel() {
        let state = makeState()

        state.generateCommitMessageWithAI()
        #expect(state.statusMessage == "No changes to summarize.")
        #expect(state.statusIsError)
        #expect(!state.isGeneratingCommitMessage)

        state.files = [makeFile("Changed.swift", xStatus: " ", yStatus: "M")]
        state.generateCommitMessageWithAI()
        #expect(state.isGeneratingCommitMessage)

        state.generateCommitMessageWithAI()
        #expect(state.isGeneratingCommitMessage)

        state.cancelCommitMessageGeneration()
        #expect(!state.isGeneratingCommitMessage)
    }

    @Test("open pull request validates title base branch current branch and in-flight state")
    func openPullRequestValidation() {
        let state = makeState()

        state.openPullRequest(.init(baseBranch: "main", title: " ", body: "", branchStrategy: .useCurrent, includeMode: .none, draft: false))
        #expect(state.openPullRequestError == "Title and target branch are required.")
        #expect(!state.isOpeningPullRequest)

        state.openPullRequestError = nil
        state.openPullRequest(.init(baseBranch: " ", title: "Title", body: "", branchStrategy: .useCurrent, includeMode: .none, draft: false))
        #expect(state.openPullRequestError == "Title and target branch are required.")
        #expect(!state.isOpeningPullRequest)

        state.openPullRequestError = nil
        state.openPullRequest(.init(baseBranch: "main", title: "Title", body: "", branchStrategy: .useCurrent, includeMode: .none, draft: false))
        #expect(state.openPullRequestError == "No current branch.")
        #expect(!state.isOpeningPullRequest)
    }

    @Test("filtered pull requests match title author branch and number")
    func filteredPullRequests() {
        let state = makeState()
        state.pullRequests = [
            makePRListItem(number: 12, title: "Improve sidebar", author: "alice", headBranch: "feature/sidebar"),
            makePRListItem(number: 42, title: "Fix terminal", author: "bob", headBranch: "bugfix/terminal"),
        ]

        state.pullRequestSearchQuery = ""
        #expect(state.filteredPullRequests.map(\.number) == [12, 42])

        state.pullRequestSearchQuery = "ALICE"
        #expect(state.filteredPullRequests.map(\.number) == [12])

        state.pullRequestSearchQuery = "terminal"
        #expect(state.filteredPullRequests.map(\.number) == [42])

        state.pullRequestSearchQuery = "12"
        #expect(state.filteredPullRequests.map(\.number) == [12])
    }

    @Test("section visibility and collapse setters persist state")
    func sectionVisibilityAndCollapse() {
        let path = uniqueProjectPath()
        VCSPersistedSettings.storeSectionVisibility(.init(changes: true, history: true, pullRequests: true), repoPath: path)
        VCSPersistedSettings.storeSectionCollapse(.init(staged: false, changes: false, history: false, pullRequests: false), repoPath: path)
        let state = VCSTabState(projectPath: path)

        state.setChangesVisible(false)
        state.setPullRequestsVisible(false)
        state.stagedCollapsed = true
        state.changesCollapsed = true
        state.historyCollapsed = true
        state.pullRequestsCollapsed = true

        let visibility = VCSPersistedSettings.loadSectionVisibility(repoPath: path)
        let collapse = VCSPersistedSettings.loadSectionCollapse(repoPath: path)

        #expect(visibility.changes == false)
        #expect(visibility.history == true)
        #expect(visibility.pullRequests == false)
        #expect(collapse.staged)
        #expect(collapse.changes)
        #expect(collapse.history)
        #expect(collapse.pullRequests)
    }

    private func makeState() -> VCSTabState {
        VCSTabState(projectPath: uniqueProjectPath())
    }

    private func uniqueProjectPath() -> String {
        NSTemporaryDirectory() + "muxy-vcs-tab-state-\(UUID().uuidString)"
    }

    private func makeFile(
        _ path: String,
        xStatus: Character,
        yStatus: Character,
        additions: Int? = 1,
        deletions: Int? = 0,
        isBinary: Bool = false
    ) -> GitStatusFile {
        GitStatusFile(
            path: path,
            oldPath: nil,
            xStatus: xStatus,
            yStatus: yStatus,
            additions: additions,
            deletions: deletions,
            isBinary: isBinary
        )
    }

    private func makePRInfo(number: Int) -> GitRepositoryService.PRInfo {
        GitRepositoryService.PRInfo(
            url: "https://example.test/pull/\(number)",
            number: number,
            state: .open,
            isDraft: false,
            baseBranch: "main",
            mergeable: true,
            mergeStateStatus: .clean,
            checks: .init(status: .success, passing: 1, failing: 0, pending: 0, total: 1),
            isCrossRepository: false
        )
    }

    private func makePRListItem(
        number: Int,
        title: String,
        author: String,
        headBranch: String
    ) -> GitRepositoryService.PRListItem {
        GitRepositoryService.PRListItem(
            number: number,
            title: title,
            author: author,
            headBranch: headBranch,
            headRefOid: "sha-\(number)",
            baseBranch: "main",
            state: .open,
            isDraft: false,
            url: "https://example.test/pull/\(number)",
            updatedAt: Date(timeIntervalSince1970: TimeInterval(number)),
            checks: .init(status: .none, passing: 0, failing: 0, pending: 0, total: 0),
            mergeable: nil,
            mergeStateStatus: .unknown
        )
    }
}
