import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("TerminalProgressStore")
struct TerminalProgressStoreTests {
    @Test("setProgress stores active progress")
    func storesProgress() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let project = UUID()
        let worktree = UUID()

        store.setProgress(
            .clamping(kind: .set, percent: 42),
            for: pane,
            worktreeKey: WorktreeKey(projectID: project, worktreeID: worktree)
        )

        #expect(store.progress(for: pane) == TerminalProgress(kind: .set, percent: 42))
        #expect(!store.isCompletionPending(for: pane))
    }

    @Test("clamps percent into 0...100")
    func clampsPercent() {
        let low = TerminalProgress.clamping(kind: .set, percent: -5)
        let high = TerminalProgress.clamping(kind: .set, percent: 250)
        let nilValue = TerminalProgress.clamping(kind: .set, percent: nil)

        #expect(low.percent == 0)
        #expect(high.percent == 100)
        #expect(nilValue.percent == nil)
    }

    @Test("transition from active to nil marks completion-pending")
    func marksCompletion() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let project = UUID()
        let key = WorktreeKey(projectID: project, worktreeID: UUID())

        store.setProgress(.clamping(kind: .set, percent: 80), for: pane, worktreeKey: key)
        store.setProgress(nil, for: pane, worktreeKey: key)

        #expect(store.progress(for: pane) == nil)
        #expect(store.isCompletionPending(for: pane))
        #expect(store.hasCompletionPending(for: project))
    }

    @Test("nil progress without prior active does not mark completion")
    func noFalseCompletion() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let project = UUID()

        store.setProgress(nil, for: pane, worktreeKey: WorktreeKey(projectID: project, worktreeID: UUID()))

        #expect(!store.isCompletionPending(for: pane))
        #expect(!store.hasCompletionPending(for: project))
    }

    @Test("clearCompletion removes pending state")
    func clearsCompletion() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let project = UUID()
        let key = WorktreeKey(projectID: project, worktreeID: UUID())

        store.setProgress(.clamping(kind: .indeterminate, percent: nil), for: pane, worktreeKey: key)
        store.setProgress(nil, for: pane, worktreeKey: key)

        store.clearCompletion(for: pane)

        #expect(!store.isCompletionPending(for: pane))
        #expect(!store.hasCompletionPending(for: project))
    }

    @Test("resetPane clears all per-pane state")
    func resetsPane() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let project = UUID()
        let key = WorktreeKey(projectID: project, worktreeID: UUID())

        store.setProgress(.clamping(kind: .set, percent: 30), for: pane, worktreeKey: key)
        store.setProgress(nil, for: pane, worktreeKey: key)

        store.resetPane(pane)

        #expect(store.progress(for: pane) == nil)
        #expect(!store.isCompletionPending(for: pane))
        #expect(!store.hasCompletionPending(for: project))
    }

    @Test("hasActiveProgress reflects running then finished state")
    func tracksActiveProgress() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let project = UUID()
        let key = WorktreeKey(projectID: project, worktreeID: UUID())

        #expect(!store.hasActiveProgress(for: project))

        store.setProgress(.clamping(kind: .set, percent: 60), for: pane, worktreeKey: key)
        #expect(store.hasActiveProgress(for: project))
        #expect(!store.hasCompletionPending(for: project))

        store.setProgress(nil, for: pane, worktreeKey: key)
        #expect(!store.hasActiveProgress(for: project))
        #expect(store.hasCompletionPending(for: project))

        store.resetPane(pane)
        #expect(!store.hasActiveProgress(for: project))
        #expect(!store.hasCompletionPending(for: project))
    }

    @Test("hasActiveProgress scopes by project")
    func activeProgressScopesByProject() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let projectA = UUID()
        let projectB = UUID()

        store.setProgress(
            .clamping(kind: .set, percent: 10),
            for: pane,
            worktreeKey: WorktreeKey(projectID: projectA, worktreeID: UUID())
        )

        #expect(store.hasActiveProgress(for: projectA))
        #expect(!store.hasActiveProgress(for: projectB))
    }

    @Test("hasCompletionPending scopes by project")
    func scopesByProject() {
        let store = TerminalProgressStore()
        let paneA = UUID()
        let projectA = UUID()
        let projectB = UUID()
        let key = WorktreeKey(projectID: projectA, worktreeID: UUID())

        store.setProgress(.clamping(kind: .set, percent: 50), for: paneA, worktreeKey: key)
        store.setProgress(nil, for: paneA, worktreeKey: key)

        #expect(store.hasCompletionPending(for: projectA))
        #expect(!store.hasCompletionPending(for: projectB))
    }

    @Test("progress and completion scope by worktree")
    func scopesByWorktree() {
        let store = TerminalProgressStore()
        let pane = UUID()
        let project = UUID()
        let worktreeA = UUID()
        let worktreeB = UUID()
        let key = WorktreeKey(projectID: project, worktreeID: worktreeA)

        store.setProgress(.clamping(kind: .set, percent: 50), for: pane, worktreeKey: key)

        #expect(store.hasActiveProgress(forWorktree: worktreeA))
        #expect(!store.hasActiveProgress(forWorktree: worktreeB))

        store.setProgress(nil, for: pane, worktreeKey: key)

        #expect(store.hasCompletionPending(forWorktree: worktreeA))
        #expect(!store.hasCompletionPending(forWorktree: worktreeB))
    }
}
