import Testing

@testable import Muxy

@Suite("CreateWorktreeSheet")
struct CreateWorktreeSheetTests {
    @Test("branch loading starts without selectable branches")
    func initialBranchLoadingState() {
        let state = WorktreeBranchLoadState()

        #expect(state.isLoading)
        #expect(state.branches.isEmpty)
        #expect(state.selectedExistingBranch.isEmpty)
        #expect(state.selectedBaseBranch.isEmpty)
    }

    @Test("loaded branches select the first existing branch and resolved default base")
    func loadedBranchesSelectDefaults() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(branches: ["feature", "main"], defaultBranch: "main")

        #expect(!state.isLoading)
        #expect(state.branches == ["feature", "main"])
        #expect(state.selectedExistingBranch == "feature")
        #expect(state.selectedBaseBranch == "main")
    }

    @Test("missing default branch falls back to the first branch")
    func missingDefaultBranchFallsBack() {
        var state = WorktreeBranchLoadState()

        state.finishLoading(branches: ["develop", "feature"], defaultBranch: "main")

        #expect(state.selectedExistingBranch == "develop")
        #expect(state.selectedBaseBranch == "develop")
    }

    @Test("loaded branches preserve existing selections")
    func loadedBranchesPreserveSelections() {
        var state = WorktreeBranchLoadState()
        state.selectedExistingBranch = "feature"
        state.selectedBaseBranch = "develop"

        state.finishLoading(
            branches: ["feature", "develop", "main"],
            defaultBranch: "main"
        )

        #expect(state.selectedExistingBranch == "feature")
        #expect(state.selectedBaseBranch == "develop")
    }

    @Test("branch loading failure leaves selection disabled")
    func branchLoadingFailure() {
        var state = WorktreeBranchLoadState()

        state.failLoading()

        #expect(!state.isLoading)
        #expect(state.branches.isEmpty)
        #expect(state.selectedExistingBranch.isEmpty)
        #expect(state.selectedBaseBranch.isEmpty)
    }
}
