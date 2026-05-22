import Testing

@testable import Muxy

@Suite("GitRepositoryService PR checkout helpers")
struct GitRepositoryServicePRCheckoutTests {
    @Test("local branch keeps PR identity and head branch hierarchy")
    func localBranchName() {
        let checkout = GitRepositoryService.PRCheckoutInfo(
            number: 42,
            headBranch: "feature/fork-pr",
            headRepositoryNameWithOwner: "alice/repo"
        )

        #expect(GitRepositoryService.localPullRequestBranchName(for: checkout) == "pr/42/feature/fork-pr")
    }

    @Test("generated names remove unsafe ref characters")
    func generatedNamesRemoveUnsafeCharacters() {
        let checkout = GitRepositoryService.PRCheckoutInfo(
            number: 42,
            headBranch: "feature/1.0 @{bad}:name",
            headRepositoryNameWithOwner: "alice/repo.name"
        )

        #expect(GitRepositoryService.localPullRequestBranchName(for: checkout) == "pr/42/feature/1-0-bad-name")
        #expect(GitRepositoryService.pullRequestRemoteName(for: checkout) == "pr-42-alice-repo-name")
    }
}
