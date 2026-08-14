import Foundation
import Testing

@testable import Muxy

@Suite("TabIndicatorPolicy")
@MainActor
struct TabIndicatorPolicyTests {
    @Test("working child overrides an idle parent")
    func workingChildOverridesIdleParent() {
        #expect(TabIndicatorPolicy.agentStatus(from: [.idle, .working]) == .working)
        #expect(TabIndicatorPolicy.agentStatus(from: [.working, .idle]) == .working)
    }

    @Test("waiting pane remains the highest tab attention state")
    func waitingPaneWins() {
        #expect(TabIndicatorPolicy.agentStatus(from: [.idle, .working, .waiting]) == .waiting)
        #expect(TabIndicatorPolicy.agentStatus(from: [.waiting, .working]) == .waiting)
    }

    @Test("idle and empty pane groups retain their stable status")
    func idleAndEmptyGroups() {
        #expect(TabIndicatorPolicy.agentStatus(from: [.idle, .idle]) == .idle)
        #expect(TabIndicatorPolicy.agentStatus(from: []) == nil)
    }

    @Test("active tabs show attention from an unfocused child")
    func activeTabsShowUnfocusedAttention() {
        #expect(TabIndicatorPolicy.showsAttentionDot(
            isActive: true,
            hasAttention: true,
            hasUnfocusedAttention: true
        ))
        #expect(!TabIndicatorPolicy.showsAttentionDot(
            isActive: true,
            hasAttention: true,
            hasUnfocusedAttention: false
        ))
        #expect(TabIndicatorPolicy.showsAttentionDot(
            isActive: false,
            hasAttention: true,
            hasUnfocusedAttention: false
        ))
    }

    @Test("completion clears only the focused related pane")
    func completionClearsFocusedRelatedPane() {
        let focusedPaneID = UUID()
        let siblingPaneID = UUID()

        #expect(TabIndicatorPolicy.completionPaneIDToClear(
            isActive: true,
            focusedPaneID: focusedPaneID,
            newlyPendingPaneIDs: [focusedPaneID, siblingPaneID]
        ) == focusedPaneID)
        #expect(TabIndicatorPolicy.completionPaneIDToClear(
            isActive: false,
            focusedPaneID: focusedPaneID,
            newlyPendingPaneIDs: [focusedPaneID, siblingPaneID]
        ) == nil)
        #expect(TabIndicatorPolicy.completionPaneIDToClear(
            isActive: true,
            focusedPaneID: focusedPaneID,
            newlyPendingPaneIDs: [siblingPaneID]
        ) == nil)
    }

    @Test("sequential sibling completions acknowledge the newly completed focused pane")
    func sequentialSiblingCompletions() {
        let store = TerminalProgressStore()
        let focusedPaneID = UUID()
        let siblingPaneID = UUID()
        let paneIDs = [focusedPaneID, siblingPaneID]
        let worktreeKey = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        func pendingPaneIDs() -> Set<UUID> {
            Set(paneIDs.filter { store.isCompletionPending(for: $0) })
        }

        store.setProgress(.clamping(kind: .indeterminate, percent: nil), for: siblingPaneID, worktreeKey: worktreeKey)
        store.setProgress(nil, for: siblingPaneID, worktreeKey: worktreeKey)
        let siblingPendingPaneIDs = pendingPaneIDs()

        #expect(TabIndicatorPolicy.newlyPendingPaneIDs(
            previous: [],
            current: siblingPendingPaneIDs
        ) == [siblingPaneID])

        store.setProgress(.clamping(kind: .indeterminate, percent: nil), for: focusedPaneID, worktreeKey: worktreeKey)
        store.setProgress(nil, for: focusedPaneID, worktreeKey: worktreeKey)
        let allPendingPaneIDs = pendingPaneIDs()
        let newlyPendingPaneIDs = TabIndicatorPolicy.newlyPendingPaneIDs(
            previous: siblingPendingPaneIDs,
            current: allPendingPaneIDs
        )
        let paneIDToClear = TabIndicatorPolicy.completionPaneIDToClear(
            isActive: true,
            focusedPaneID: focusedPaneID,
            newlyPendingPaneIDs: newlyPendingPaneIDs
        )

        #expect(newlyPendingPaneIDs == [focusedPaneID])
        #expect(paneIDToClear == focusedPaneID)
        if let paneIDToClear {
            store.clearCompletion(for: paneIDToClear)
        }
        #expect(!store.isCompletionPending(for: focusedPaneID))
        #expect(store.isCompletionPending(for: siblingPaneID))
    }
}
