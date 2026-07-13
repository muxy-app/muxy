import Testing

@testable import Muxy

@Suite("Tab focused repository state")
struct TabFocusedRepositoryStateTests {
    @Test("keeps resolved pull request state while refreshing the same revision")
    func keepsResolvedStateForSameRevision() {
        let identity = pullRequestIdentity(branch: "feature/toolbar", headOID: "abc123")

        let state = TabFocusedRepositoryState.pullRequestStateForRefresh(
            current: .noPullRequest,
            resolvedIdentity: identity,
            requestedIdentity: identity
        )

        #expect(state == .noPullRequest)
    }

    @Test("hides stale pull request state when the branch changes")
    func hidesStaleStateForBranchChange() {
        let state = TabFocusedRepositoryState.pullRequestStateForRefresh(
            current: .noPullRequest,
            resolvedIdentity: pullRequestIdentity(branch: "feature/old", headOID: "abc123"),
            requestedIdentity: pullRequestIdentity(branch: "feature/new", headOID: "def456")
        )

        #expect(state == .loading)
    }

    @Test("hides stale pull request state when the head changes")
    func hidesStaleStateForHeadChange() {
        let state = TabFocusedRepositoryState.pullRequestStateForRefresh(
            current: .unavailable,
            resolvedIdentity: pullRequestIdentity(branch: "feature/toolbar", headOID: "abc123"),
            requestedIdentity: pullRequestIdentity(branch: "feature/toolbar", headOID: "def456")
        )

        #expect(state == .loading)
    }

    @Test("hides stale pull request state when the repository changes")
    func hidesStaleStateForRepositoryChange() {
        let resolvedIdentity = TabFocusedRepositoryState.PullRequestIdentity(
            repositoryKey: "local|/projects/old",
            branch: "feature/toolbar",
            headOID: "abc123"
        )
        let requestedIdentity = TabFocusedRepositoryState.PullRequestIdentity(
            repositoryKey: "local|/projects/new",
            branch: "feature/toolbar",
            headOID: "abc123"
        )

        let state = TabFocusedRepositoryState.pullRequestStateForRefresh(
            current: .noPullRequest,
            resolvedIdentity: resolvedIdentity,
            requestedIdentity: requestedIdentity
        )

        #expect(state == .loading)
    }

    private func pullRequestIdentity(
        branch: String,
        headOID: String
    ) -> TabFocusedRepositoryState.PullRequestIdentity {
        TabFocusedRepositoryState.PullRequestIdentity(
            repositoryKey: "local|/projects/app",
            branch: branch,
            headOID: headOID
        )
    }
}
