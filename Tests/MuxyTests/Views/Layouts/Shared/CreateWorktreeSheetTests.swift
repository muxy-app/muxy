import Testing

@testable import Muxy

@Suite("CreateWorktreeSheet")
struct CreateWorktreeSheetTests {
    @Test("branch loading starts without selectable branches")
    func initialBranchLoadingState() {
        let state = WorktreeBranchLoadState()

        #expect(state.isLoading)
        #expect(state.branchOptions.isEmpty)
        #expect(state.baseBranchOptions.isEmpty)
        #expect(state.selectedExistingBranch.isEmpty)
        #expect(state.selectedBaseBranch.isEmpty)
    }

    @Test("loaded branches select the first existing branch and resolved default base")
    func loadedBranchesSelectDefaults() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(
            branches: ["develop", "feature", "release"],
            defaultBranch: "release",
            remoteDefaultBranchReference: nil,
            currentBranch: "feature"
        )

        #expect(!state.isLoading)
        #expect(state.branchOptions.map(\.name) == ["develop", "feature", "release"])
        #expect(state.branchOptions.map(\.reference) == ["develop", "feature", "release"])
        #expect(state.baseBranchOptions == state.branchOptions)
        #expect(state.selectedExistingBranch == "develop")
        #expect(state.selectedBaseBranch == "release")
    }

    @Test("remote-tracking default branch remains available as a base")
    func remoteTrackingDefaultBranchIsAvailableAsBase() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(
            branches: ["basic-ui", "develop"],
            defaultBranch: "main",
            remoteDefaultBranchReference: "refs/remotes/origin/main",
            currentBranch: "basic-ui"
        )

        #expect(state.selectedExistingBranch == "basic-ui")
        #expect(state.branchOptions.map(\.name) == ["basic-ui", "develop"])
        #expect(state.baseBranchOptions.map(\.name) == ["main", "basic-ui", "develop"])
        #expect(state.baseBranchOptions.map(\.reference) == ["refs/remotes/origin/main", "basic-ui", "develop"])
        #expect(state.selectedBaseBranch == "refs/remotes/origin/main")
    }

    @Test("current branch is preferred before conventional branch fallbacks")
    func currentBranchPrecedesConventionalFallbacks() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(
            branches: ["basic-ui", "develop"],
            defaultBranch: "main",
            remoteDefaultBranchReference: nil,
            currentBranch: "basic-ui"
        )

        #expect(state.selectedBaseBranch == "basic-ui")
    }

    @Test("conventional branch fallback order is develop, main, then master")
    func conventionalBranchFallbackOrder() {
        var developState = WorktreeBranchLoadState()
        var mainState = WorktreeBranchLoadState()
        var masterState = WorktreeBranchLoadState()

        developState.finishLoading(
            branches: ["master", "main", "develop"],
            defaultBranch: nil,
            remoteDefaultBranchReference: nil,
            currentBranch: nil
        )
        mainState.finishLoading(
            branches: ["master", "main"],
            defaultBranch: nil,
            remoteDefaultBranchReference: nil,
            currentBranch: nil
        )
        masterState.finishLoading(
            branches: ["feature", "master"],
            defaultBranch: nil,
            remoteDefaultBranchReference: nil,
            currentBranch: nil
        )

        #expect(developState.selectedBaseBranch == "develop")
        #expect(mainState.selectedBaseBranch == "main")
        #expect(masterState.selectedBaseBranch == "master")
    }

    @Test("missing default and current branches leave the base unselected")
    func missingDefaultAndCurrentBranchesLeaveBaseUnselected() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(
            branches: ["basic-ui", "feature"],
            defaultBranch: "main",
            remoteDefaultBranchReference: nil,
            currentBranch: nil
        )

        #expect(state.selectedExistingBranch == "basic-ui")
        #expect(state.selectedBaseBranch.isEmpty)
    }

    @Test("loaded branches preserve existing selections")
    func loadedBranchesPreserveSelections() {
        var state = WorktreeBranchLoadState()
        state.selectedExistingBranch = "feature"
        state.selectedBaseBranch = "develop"

        state.finishLoading(
            branches: ["feature", "develop", "main"],
            defaultBranch: "main",
            remoteDefaultBranchReference: nil,
            currentBranch: "main"
        )

        #expect(state.selectedExistingBranch == "feature")
        #expect(state.selectedBaseBranch == "develop")
    }

    @Test("branch loading failure leaves selection disabled")
    func branchLoadingFailure() {
        var state = WorktreeBranchLoadState()

        state.failLoading()

        #expect(!state.isLoading)
        #expect(state.branchOptions.isEmpty)
        #expect(state.baseBranchOptions.isEmpty)
        #expect(state.selectedExistingBranch.isEmpty)
        #expect(state.selectedBaseBranch.isEmpty)
    }
}
