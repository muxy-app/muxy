import Foundation
import Testing

@testable import Muxy

@Suite("Rich input state")
@MainActor
struct RichInputStateTests {
    @Test("clear resets all content fields")
    func clearResetsAllFields() {
        let state = RichInputState()
        state.text = "draft text"
        state.fileAttachments = [URL(fileURLWithPath: "/tmp/notes.txt")]
        state.imageAttachments = [URL(fileURLWithPath: "/tmp/image.png")]
        state.imagePlaceholderCounter = 7
        state.focusVersion = 5

        state.clear()

        #expect(state.text == "")
        #expect(state.fileAttachments.isEmpty)
        #expect(state.imageAttachments.isEmpty)
        #expect(state.imagePlaceholderCounter == 0)
        #expect(state.focusVersion == 5)
    }

    @Test("conditional clear removes an unchanged submitted draft")
    func conditionalClearRemovesUnchangedDraft() {
        let state = RichInputState()
        state.text = "submitted text"
        let submittedRevision = state.draftRevision

        #expect(state.clear(ifUnchangedSince: submittedRevision))
        #expect(state.draft == .empty)
    }

    @Test("conditional clear preserves edits made during submission")
    func conditionalClearPreservesNewerEdits() {
        let state = RichInputState()
        state.text = "submitted text"
        let submittedRevision = state.draftRevision
        state.text = "newer draft"

        #expect(!state.clear(ifUnchangedSince: submittedRevision))
        #expect(state.text == "newer draft")
    }

    @Test("conditional clear preserves edits restored to the submitted value")
    func conditionalClearPreservesRestoredEdits() {
        let state = RichInputState()
        state.text = "submitted text"
        let submittedDraft = state.draft
        let submittedRevision = state.draftRevision
        state.text = "intermediate edit"
        state.text = "submitted text"

        #expect(state.draft == submittedDraft)
        #expect(!state.clear(ifUnchangedSince: submittedRevision))
        #expect(state.text == "submitted text")
    }
}
