import Testing

@testable import Muxy

@Suite("RepositoryToolbarPresentation")
struct RepositoryToolbarPresentationTests {
    @Test("hides only when there is no repository")
    func hidesWithoutRepository() {
        let state = RepositoryToolbarPresentation.contentState(
            hasRepository: false,
            hasSummary: false,
            error: nil
        )

        #expect(state == .hidden)
    }

    @Test("materializes loading content before the first repository result")
    func loadsBeforeFirstResult() {
        let state = RepositoryToolbarPresentation.contentState(
            hasRepository: true,
            hasSummary: false,
            error: nil
        )

        #expect(state == .loading)
    }

    @Test("surfaces repository errors instead of becoming empty")
    func surfacesRepositoryErrors() {
        let state = RepositoryToolbarPresentation.contentState(
            hasRepository: true,
            hasSummary: false,
            error: "Not a Git repository"
        )

        #expect(state == .unavailable("Not a Git repository"))
    }

    @Test("shows repository content after a summary loads")
    func showsLoadedRepository() {
        let state = RepositoryToolbarPresentation.contentState(
            hasRepository: true,
            hasSummary: true,
            error: "Stale error"
        )

        #expect(state == .ready)
    }

    @Test("hides worktree removal without an active worktree")
    func hidesWorktreeRemovalWithoutWorktree() {
        let state = RepositoryToolbarPresentation.worktreeRemovalState(
            worktree: nil,
            isPreparing: false,
            isRemoving: false
        )

        #expect(state == .hidden)
    }

    @Test("hides worktree removal for the primary worktree")
    func hidesPrimaryWorktreeRemoval() {
        let state = RepositoryToolbarPresentation.worktreeRemovalState(
            worktree: worktree(isPrimary: true),
            isPreparing: false,
            isRemoving: false
        )

        #expect(state == .hidden)
    }

    @Test("makes secondary worktree removal available")
    func makesSecondaryWorktreeRemovalAvailable() {
        let state = RepositoryToolbarPresentation.worktreeRemovalState(
            worktree: worktree(isPrimary: false),
            isPreparing: false,
            isRemoving: false
        )

        #expect(state == .available)
    }

    @Test("surfaces secondary worktree removal progress")
    func surfacesSecondaryWorktreeRemovalProgress() {
        let state = RepositoryToolbarPresentation.worktreeRemovalState(
            worktree: worktree(isPrimary: false),
            isPreparing: false,
            isRemoving: true
        )

        #expect(state == .removing)
    }

    @Test("surfaces secondary worktree removal preparation")
    func surfacesSecondaryWorktreeRemovalPreparation() {
        let state = RepositoryToolbarPresentation.worktreeRemovalState(
            worktree: worktree(isPrimary: false),
            isPreparing: true,
            isRemoving: false
        )

        #expect(state == .preparing)
    }

    private func worktree(isPrimary: Bool) -> Worktree {
        Worktree(
            name: isPrimary ? "primary" : "feature",
            path: isPrimary ? "/projects/app" : "/worktrees/feature",
            isPrimary: isPrimary
        )
    }
}

@Suite("PullRequestPresentation")
struct PullRequestPresentationTests {
    @Test("unstable PR with pending checks reports checks running")
    func unstablePendingChecks() throws {
        let presentation = try #require(PRMergeabilityPresentation.make(info: prInfo(
            mergeStateStatus: .unstable,
            checks: GitRepositoryService.PRChecks(status: .pending, passing: 1, failing: 0, pending: 1, total: 2)
        )))

        #expect(presentation.text == "Checks running")
        #expect(presentation.tone == .warning)
    }

    @Test("unstable PR with failing checks remains mergeable with confirmation")
    func unstableFailingChecks() throws {
        let info = prInfo(
            mergeStateStatus: .unstable,
            checks: GitRepositoryService.PRChecks(status: .failure, passing: 1, failing: 1, pending: 0, total: 2)
        )
        let presentation = try #require(PRMergeabilityPresentation.make(info: info))
        let availability = PRMergeAvailability.make(info: info)

        #expect(presentation.text == "Checks failing")
        #expect(presentation.tone == .warning)
        #expect(availability.isEnabled)
        #expect(availability.help.contains("five-second merge confirmation"))
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

    @Test(
        "unsafe merge states are disabled",
        arguments: [
            GitRepositoryService.PRMergeStateStatus.dirty,
            .behind,
            .blocked,
            .draft,
        ]
    )
    func unsafeMergeStatesAreDisabled(state: GitRepositoryService.PRMergeStateStatus) {
        #expect(!PRMergeAvailability.make(info: prInfo(mergeStateStatus: state)).isEnabled)
    }

    @Test("closed pull request cannot be merged")
    func closedPullRequestCannotBeMerged() {
        #expect(!PRMergeAvailability.make(info: prInfo(state: .closed, mergeStateStatus: .clean)).isEnabled)
    }

    @Test("draft pull request cannot be merged regardless of merge state")
    func draftPullRequestCannotBeMerged() throws {
        let info = prInfo(isDraft: true, mergeStateStatus: .clean)
        let presentation = try #require(PRMergeabilityPresentation.make(info: info))

        #expect(!PRMergeAvailability.make(info: info).isEnabled)
        #expect(presentation.text == "Draft")
        #expect(presentation.tone == .muted)
    }

    private func prInfo(
        state: GitRepositoryService.PRState = .open,
        isDraft: Bool = false,
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
            state: state,
            isDraft: isDraft,
            baseBranch: "main",
            mergeable: mergeable,
            mergeStateStatus: mergeStateStatus,
            checks: checks,
            isCrossRepository: false
        )
    }
}

@Suite("PullRequestActionConfirmation")
struct PullRequestActionConfirmationTests {
    @Test("first click arms the requested action")
    func firstClickArmsAction() {
        let action = PullRequestActionConfirmation.Kind.merge(.squash)

        #expect(PullRequestActionConfirmation.activation(pending: nil, requested: action) == .arm(action))
        #expect(PullRequestActionConfirmation.duration == 5)
    }

    @Test("second click confirms the armed action")
    func secondClickConfirmsAction() {
        let action = PullRequestActionConfirmation.Kind.close
        let pending = PullRequestActionConfirmation.Pending(kind: action)

        #expect(PullRequestActionConfirmation.activation(pending: pending, requested: action) == .confirm)
    }

    @Test("a different action replaces the armed action")
    func differentActionRearmsConfirmation() {
        let pending = PullRequestActionConfirmation.Pending(kind: .merge(.squash))
        let requested = PullRequestActionConfirmation.Kind.close

        #expect(PullRequestActionConfirmation.activation(pending: pending, requested: requested) == .arm(requested))
    }

    @Test("a different merge method replaces the armed merge")
    func differentMergeMethodRearmsConfirmation() {
        let pending = PullRequestActionConfirmation.Pending(kind: .merge(.squash))
        let requested = PullRequestActionConfirmation.Kind.merge(.rebase)

        #expect(PullRequestActionConfirmation.activation(pending: pending, requested: requested) == .arm(requested))
    }

    @Test("rearming the same action creates a distinct confirmation")
    func rearmingSameActionCreatesDistinctConfirmation() {
        let action = PullRequestActionConfirmation.Kind.close
        let first = PullRequestActionConfirmation.Pending(kind: action)
        let replacement = PullRequestActionConfirmation.Pending(kind: action)

        #expect(first != replacement)
        #expect(first.kind == replacement.kind)
    }

    @Test("cancel clears the armed confirmation")
    func cancelClearsConfirmation() {
        var state = PullRequestActionConfirmation.State()
        let pending = state.arm(.merge(.squash))

        #expect(state.pending == pending)

        state.cancel()

        #expect(state.pending == nil)
    }

    @Test("repository head changes invalidate the confirmation context")
    func headChangeInvalidatesContext() {
        let pullRequest = GitRepositoryService.PRInfo(
            url: "https://github.com/acme/app/pull/1",
            number: 1,
            state: .open,
            isDraft: false,
            baseBranch: "main",
            mergeable: true,
            mergeStateStatus: .clean,
            checks: GitRepositoryService.PRChecks(
                status: .success,
                passing: 2,
                failing: 0,
                pending: 0,
                total: 2
            ),
            isCrossRepository: false
        )
        let original = PullRequestActionConfirmation.Context(
            repositoryID: "repository",
            branch: "feature",
            headOID: "abc",
            pullRequest: pullRequest
        )
        let changed = PullRequestActionConfirmation.Context(
            repositoryID: "repository",
            branch: "feature",
            headOID: "def",
            pullRequest: pullRequest
        )

        #expect(original != changed)
    }
}
