import Testing

@testable import Muxy

@Suite("TerminalFocusRestorationPolicy")
struct TerminalFocusRestorationPolicyTests {
    @Test("focused terminal reclaims focus after its creation overlay closes")
    func focusedTerminalReclaimsFocusAfterCreationOverlayCloses() {
        #expect(TerminalFocusRestorationPolicy.shouldClaimFocus(
            focused: true,
            wasFocused: true,
            wasOverlayActive: true
        ))
    }

    @Test("focused terminal claims focus when it becomes selected")
    func focusedTerminalClaimsFocusWhenSelected() {
        #expect(TerminalFocusRestorationPolicy.shouldClaimFocus(
            focused: true,
            wasFocused: false,
            wasOverlayActive: false
        ))
    }

    @Test("unfocused terminal does not claim focus")
    func unfocusedTerminalDoesNotClaimFocus() {
        #expect(!TerminalFocusRestorationPolicy.shouldClaimFocus(
            focused: false,
            wasFocused: true,
            wasOverlayActive: true
        ))
    }

    @Test("unchanged focused terminal does not repeatedly claim focus")
    func unchangedFocusedTerminalDoesNotRepeatedlyClaimFocus() {
        #expect(!TerminalFocusRestorationPolicy.shouldClaimFocus(
            focused: true,
            wasFocused: true,
            wasOverlayActive: false
        ))
    }
}
