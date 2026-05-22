import Foundation
import Testing

@testable import Muxy

@Suite("RichInputController")
@MainActor
struct RichInputControllerTests {
    @Test("state(for:) lazily creates and caches a RichInputState per worktree key")
    func stateLazilyCachesPerKey() {
        let controller = RichInputController()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        let first = controller.state(for: key)
        let second = controller.state(for: key)

        #expect(first === second)
        #expect(controller.existingState(for: key) === first)
    }

    @Test("existingState(for:) returns nil until state(for:) is invoked")
    func existingStateReturnsNilUntilCreated() {
        let controller = RichInputController()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        #expect(controller.existingState(for: key) == nil)

        _ = controller.state(for: key)

        #expect(controller.existingState(for: key) != nil)
    }

    @Test("prune drops states whose worktree keys are no longer valid")
    func pruneRemovesInvalidKeys() {
        let controller = RichInputController()
        let keep = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let drop = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        _ = controller.state(for: keep)
        _ = controller.state(for: drop)

        controller.prune(validKeys: [keep])

        #expect(controller.existingState(for: keep) != nil)
        #expect(controller.existingState(for: drop) == nil)
    }

    @Test("appendMarkdown writes into an empty state without a leading newline")
    func appendIntoEmptyStateHasNoLeadingNewline() {
        let controller = RichInputController()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        controller.appendMarkdown("hello", for: key)

        let state = controller.state(for: key)
        #expect(state.text == "hello")
        #expect(state.focusVersion == 1)
    }

    @Test("appendMarkdown separates existing text with a single newline")
    func appendIntoExistingStateAddsSeparator() {
        let controller = RichInputController()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let state = controller.state(for: key)
        state.text = "first"

        controller.appendMarkdown("second", for: key)

        #expect(state.text == "first\nsecond")
    }

    @Test("appendMarkdown reuses existing trailing newline rather than doubling it")
    func appendDoesNotDoubleNewline() {
        let controller = RichInputController()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let state = controller.state(for: key)
        state.text = "first\n"

        controller.appendMarkdown("second", for: key)

        #expect(state.text == "first\nsecond")
    }

    @Test("appendMarkdown ignores whitespace-only input")
    func appendIgnoresEmptyContent() {
        let controller = RichInputController()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let state = controller.state(for: key)
        state.text = "first"

        controller.appendMarkdown("   \n  ", for: key)

        #expect(state.text == "first")
        #expect(state.focusVersion == 0)
    }
}
