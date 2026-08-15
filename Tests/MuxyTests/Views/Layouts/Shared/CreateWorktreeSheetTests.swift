import Testing

@testable import Muxy

@Suite("CreateWorktreeSheet")
struct CreateWorktreeSheetTests {
    @Test("branch loading starts without selectable branches")
    func initialBranchLoadingState() {
        let state = WorktreeBranchLoadState()

        #expect(state.isLoading)
        #expect(state.branchOptions.isEmpty)
        #expect(state.selectedExistingBranch.isEmpty)
        #expect(state.selectedBaseBranch.isEmpty)
    }

    @Test("loaded branches select the first existing branch and resolved default base")
    func loadedBranchesSelectDefaults() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(
            branches: ["develop", "feature", "release"],
            defaultBranch: "release",
            currentBranch: "feature"
        )

        #expect(!state.isLoading)
        #expect(state.branchOptions.map(\.name) == ["develop", "feature", "release"])
        #expect(state.branchOptions.map(\.id) == ["develop", "feature", "release"])
        #expect(state.selectedExistingBranch == "develop")
        #expect(state.selectedBaseBranch == "release")
    }

    @Test("missing local default branch falls back to develop before the current branch")
    func missingLocalDefaultBranchFallsBackToDevelop() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(branches: ["basic-ui", "develop"], defaultBranch: "main", currentBranch: "basic-ui")

        #expect(state.selectedExistingBranch == "basic-ui")
        #expect(state.selectedBaseBranch == "develop")
    }

    @Test("conventional branch fallback order is develop, main, then master")
    func conventionalBranchFallbackOrder() {
        var developState = WorktreeBranchLoadState()
        var mainState = WorktreeBranchLoadState()
        var masterState = WorktreeBranchLoadState()

        developState.finishLoading(
            branches: ["master", "main", "develop"],
            defaultBranch: nil,
            currentBranch: "master"
        )
        mainState.finishLoading(branches: ["master", "main"], defaultBranch: nil, currentBranch: "master")
        masterState.finishLoading(branches: ["feature", "master"], defaultBranch: nil, currentBranch: "feature")

        #expect(developState.selectedBaseBranch == "develop")
        #expect(mainState.selectedBaseBranch == "main")
        #expect(masterState.selectedBaseBranch == "master")
    }

    @Test("missing default and current branches leave the base unselected")
    func missingDefaultAndCurrentBranchesLeaveBaseUnselected() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(branches: ["basic-ui", "feature"], defaultBranch: "main", currentBranch: nil)

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
        #expect(state.selectedExistingBranch.isEmpty)
        #expect(state.selectedBaseBranch.isEmpty)
    }
}
