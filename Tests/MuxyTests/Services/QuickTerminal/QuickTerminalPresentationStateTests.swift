import Testing

@testable import Muxy

@Suite("Quick terminal presentation state")
struct QuickTerminalPresentationStateTests {
    @Test("show and hide complete in order")
    func showAndHide() throws {
        var state = QuickTerminalPresentationState()
        let showRequest = state.requestVisibility(true)
        let show = try #require(showRequest)

        #expect(state.phase == .showing)
        let didCompleteShow = state.complete(show)
        #expect(didCompleteShow)
        #expect(state.phase == .visible)

        let hideRequest = state.requestVisibility(false)
        let hide = try #require(hideRequest)

        #expect(state.phase == .hiding)
        let didCompleteHide = state.complete(hide)
        #expect(didCompleteHide)
        #expect(state.phase == .hidden)
    }

    @Test("rapid reversal ignores stale completion")
    func rapidReversal() throws {
        var state = QuickTerminalPresentationState()
        let firstShowRequest = state.requestVisibility(true)
        let firstShow = try #require(firstShowRequest)
        let hideRequest = state.requestVisibility(false)
        let hide = try #require(hideRequest)
        let secondShowRequest = state.requestVisibility(true)
        let secondShow = try #require(secondShowRequest)
        let didCompleteFirstShow = state.complete(firstShow)
        let didCompleteHide = state.complete(hide)

        #expect(!didCompleteFirstShow)
        #expect(!didCompleteHide)
        #expect(state.phase == .showing)
        let didCompleteSecondShow = state.complete(secondShow)
        #expect(didCompleteSecondShow)
        #expect(state.phase == .visible)
    }

    @Test("duplicate target is a no-op")
    func duplicateTarget() {
        var state = QuickTerminalPresentationState()
        let initialHide = state.requestVisibility(false)
        let show = state.requestVisibility(true)
        let duplicateShow = state.requestVisibility(true)

        #expect(initialHide == nil)
        #expect(show != nil)
        #expect(duplicateShow == nil)
    }
}
