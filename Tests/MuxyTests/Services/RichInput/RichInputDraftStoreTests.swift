import Foundation
import Testing

@testable import Muxy

@Suite("Rich input draft store")
@MainActor
struct RichInputDraftStoreTests {
    @Test("clearing state and scheduling the empty draft removes the persisted draft")
    func clearingStateRemovesPersistedDraft() throws {
        let url = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-draft-store-\(UUID().uuidString).json")
        try? FileManager.default.removeItem(at: url)
        let worktreeKey = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        let savingStore = RichInputDraftStore(fileURL: url)
        let state = RichInputState()
        state.text = "remember me"
        state.fileAttachments = [URL(fileURLWithPath: "/tmp/notes.txt")]

        savingStore.scheduleSave(state.draft, for: worktreeKey)
        savingStore.flush()
        #expect(savingStore.draft(for: worktreeKey)?.text == "remember me")

        state.clear()
        savingStore.scheduleSave(state.draft, for: worktreeKey)
        savingStore.flush()
        #expect(savingStore.draft(for: worktreeKey) == nil)

        let reopenedStore = RichInputDraftStore(fileURL: url)
        #expect(reopenedStore.draft(for: worktreeKey) == nil)

        try? FileManager.default.removeItem(at: url)
    }

    @Test("targeted clearing removes only the closed Composer draft")
    func targetedClearingRemovesOnlyClosedDraft() throws {
        let url = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-targeted-draft-clear-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: url) }
        let closedKey = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let activeKey = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let closedState = RichInputState()
        let activeState = RichInputState()
        closedState.text = "closed draft"
        activeState.text = "active draft"
        let store = RichInputDraftStore(fileURL: url)
        store.scheduleSave(closedState.draft, for: closedKey)
        store.scheduleSave(activeState.draft, for: activeKey)
        store.flush()

        RichInputDraftClearer.clear(
            target: RichInputPresentationTarget(worktreeKey: closedKey, paneID: UUID()),
            states: [closedKey: closedState, activeKey: activeState],
            store: store
        )
        store.flush()

        #expect(closedState.draft == .empty)
        #expect(activeState.text == "active draft")
        #expect(store.draft(for: closedKey) == nil)
        #expect(store.draft(for: activeKey)?.text == "active draft")
    }
}
