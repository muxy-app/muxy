import Testing

@testable import Muxy

@Suite("PRMergeabilityPresentation")
struct PRMergeabilityPresentationTests {
    @Test("unstable PR with pending checks reports checks running")
    func unstablePendingChecks() throws {
        let presentation = try #require(PRMergeabilityPresentation.make(info: prInfo(
            mergeStateStatus: .unstable,
            checks: GitRepositoryService.PRChecks(status: .pending, passing: 1, failing: 0, pending: 1, total: 2)
        )))

        #expect(presentation.text == "Yes (checks running)")
        #expect(presentation.tone == .positive)
    }

    @Test("unstable PR with failing checks reports checks failing")
    func unstableFailingChecks() throws {
        let presentation = try #require(PRMergeabilityPresentation.make(info: prInfo(
            mergeStateStatus: .unstable,
            checks: GitRepositoryService.PRChecks(status: .failure, passing: 1, failing: 1, pending: 0, total: 2)
        )))

        #expect(presentation.text == "Yes (checks failing)")
        #expect(presentation.tone == .warning)
    }

    @Test("unknown merge state falls back to mergeable value")
    func unknownMergeStateFallsBackToMergeableValue() throws {
        let presentation = try #require(PRMergeabilityPresentation.make(info: prInfo(
            mergeable: false,
            mergeStateStatus: .unknown
        )))

        #expect(presentation.text == "Conflicts")
        #expect(presentation.tone == .negative)
    }

    private func prInfo(
        mergeable: Bool? = true,
        mergeStateStatus: GitRepositoryService.PRMergeStateStatus,
        checks: GitRepositoryService.PRChecks = GitRepositoryService.PRChecks(
            status: .none,
            passing: 0,
            failing: 0,
            pending: 0,
            total: 0
        )
    ) -> GitRepositoryService.PRInfo {
        GitRepositoryService.PRInfo(
            url: "https://github.com/acme/app/pull/1",
            number: 1,
            state: .open,
            isDraft: false,
            baseBranch: "main",
            mergeable: mergeable,
            mergeStateStatus: mergeStateStatus,
            checks: checks,
            isCrossRepository: false
        )
    }
}
