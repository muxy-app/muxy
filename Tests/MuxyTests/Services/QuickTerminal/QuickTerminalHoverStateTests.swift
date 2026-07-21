import Testing

@testable import Muxy

@Suite("Quick Terminal hover state")
struct QuickTerminalHoverStateTests {
    @Test("entering an armed zone starts the dwell timer")
    func enterStartsDwell() {
        var state = QuickTerminalHoverState()

        #expect(state.handle(.pointerEntered) == .startDwellTimer)
        #expect(state.mode == .dwelling)
        #expect(state.pointerInside)
    }

    @Test("exiting during the dwell cancels the timer and re-arms")
    func exitDuringDwellCancels() {
        var state = QuickTerminalHoverState()
        _ = state.handle(.pointerEntered)

        #expect(state.handle(.pointerExited) == .cancelDwellTimer)
        #expect(state.mode == .armed)
        #expect(!state.pointerInside)
    }

    @Test("the dwell elapsing requests opening")
    func dwellElapsedOpens() {
        var state = QuickTerminalHoverState()
        _ = state.handle(.pointerEntered)

        #expect(state.handle(.dwellElapsed) == .requestOpen)
        #expect(state.mode == .open)
    }

    @Test("the dwell elapsing does nothing when not dwelling")
    func dwellElapsedIgnoredWhenArmed() {
        var state = QuickTerminalHoverState()

        #expect(state.handle(.dwellElapsed) == .none)
        #expect(state.mode == .armed)
    }

    @Test("opening mid-dwell cancels the pending timer")
    func openMidDwellCancels() {
        var state = QuickTerminalHoverState()
        _ = state.handle(.pointerEntered)

        #expect(state.handle(.terminalOpened) == .cancelDwellTimer)
        #expect(state.mode == .open)
    }

    @Test("closing with the pointer inside disarms until it leaves once")
    func closeInsideDisarms() {
        var state = QuickTerminalHoverState()
        _ = state.handle(.pointerEntered)
        _ = state.handle(.terminalOpened)

        #expect(state.handle(.terminalClosed(pointerInside: true)) == .none)
        #expect(state.mode == .disarmed)

        #expect(state.handle(.pointerEntered) == .none)
        #expect(state.mode == .disarmed)

        #expect(state.handle(.pointerExited) == .none)
        #expect(state.mode == .armed)

        #expect(state.handle(.pointerEntered) == .startDwellTimer)
        #expect(state.mode == .dwelling)
    }

    @Test("closing with the pointer outside re-arms immediately")
    func closeOutsideRearms() {
        var state = QuickTerminalHoverState()
        _ = state.handle(.pointerEntered)
        _ = state.handle(.terminalOpened)

        #expect(state.handle(.terminalClosed(pointerInside: false)) == .none)
        #expect(state.mode == .armed)
        #expect(state.handle(.pointerEntered) == .startDwellTimer)
    }

    @Test("resetting reflects the pointer position")
    func resetReflectsPointer() {
        var state = QuickTerminalHoverState()

        state.reset(pointerInside: true)
        #expect(state.mode == .disarmed)
        #expect(state.pointerInside)

        state.reset(pointerInside: false)
        #expect(state.mode == .armed)
        #expect(!state.pointerInside)
    }
}
