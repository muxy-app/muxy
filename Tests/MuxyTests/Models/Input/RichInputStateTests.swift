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
}
