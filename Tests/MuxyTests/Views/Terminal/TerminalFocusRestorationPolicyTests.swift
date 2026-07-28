import Foundation
import Testing

@testable import Muxy

@Suite("TerminalFocusRestorationPolicy")
struct TerminalFocusRestorationPolicyTests {
    @Test("terminal bridge focus history resets when its pane changes")
    @MainActor
    func terminalBridgeFocusHistoryResetsForNewPane() {
        let coordinator = TerminalBridge.Coordinator()
        let firstPaneID = UUID()
        let secondPaneID = UUID()

        let firstPrevious = coordinator.transition(
            paneID: firstPaneID,
            focused: true,
            overlayActive: false
        )
        let repeatedPrevious = coordinator.transition(
            paneID: firstPaneID,
            focused: true,
            overlayActive: false
        )
        let secondPrevious = coordinator.transition(
            paneID: secondPaneID,
            focused: true,
            overlayActive: false
        )
        let reclaimedPrevious = coordinator.transition(
            paneID: secondPaneID,
            focused: true,
            overlayActive: false,
            reset: true
        )

        #expect(firstPrevious == .init(focused: false, overlayActive: false))
        #expect(repeatedPrevious == .init(focused: true, overlayActive: false))
        #expect(secondPrevious == .init(focused: false, overlayActive: false))
        #expect(reclaimedPrevious == .init(focused: false, overlayActive: false))
    }

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

    @Test("reparented unfocused terminal releases native focus")
    func reparentedUnfocusedTerminalReleasesNativeFocus() {
        #expect(TerminalFocusRestorationPolicy.shouldReleaseFocus(
            focused: false,
            wasFocused: false,
            attachmentChanged: true
        ))
    }

    @Test("unchanged unfocused terminal does not repeatedly release focus")
    func unchangedUnfocusedTerminalDoesNotRepeatedlyReleaseFocus() {
        #expect(!TerminalFocusRestorationPolicy.shouldReleaseFocus(
            focused: false,
            wasFocused: false,
            attachmentChanged: false
        ))
    }
}
