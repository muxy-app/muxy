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
}
