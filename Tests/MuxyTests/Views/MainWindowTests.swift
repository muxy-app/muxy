import Testing
@testable import Muxy

@Suite("OverlayEscapeDecision")
struct OverlayEscapeDecisionTests {
    @Test func consumesEscapeWhenOverlayActive() {
        #expect(OverlayEscapeDecision.shouldConsume(isOverlayActive: true, keyCode: 53))
    }

    @Test func passesEscapeThroughWhenOverlayInactive() {
        #expect(!OverlayEscapeDecision.shouldConsume(isOverlayActive: false, keyCode: 53))
    }

    @Test func passesNonEscapeThroughWhenOverlayActive() {
        #expect(!OverlayEscapeDecision.shouldConsume(isOverlayActive: true, keyCode: 36))
    }

    @Test func passesNonEscapeThroughWhenOverlayInactive() {
        #expect(!OverlayEscapeDecision.shouldConsume(isOverlayActive: false, keyCode: 36))
    }
}

@Suite("Main window modal policy")
struct MainWindowModalPolicyTests {
    @Test func permitsAnInactiveOverlay() {
        #expect(MainWindowModalPolicy.canPresent(.composer, active: []))
    }

    @Test func permitsTheAlreadyActiveOverlay() {
        #expect(MainWindowModalPolicy.canPresent(.composer, active: [.composer]))
    }

    @Test func rejectsPresentationOverAnotherOverlay() {
        #expect(!MainWindowModalPolicy.canPresent(.composer, active: [.terminalOmnibox]))
        #expect(!MainWindowModalPolicy.canPresent(.projectPicker, active: [.composer]))
    }

    @Test func resolvesTheVisuallyTopmostOverlay() {
        #expect(MainWindowModalPolicy.topmost(in: [.composer, .terminalOmnibox]) == .terminalOmnibox)
        #expect(MainWindowModalPolicy.topmost(in: [.composer, .extensionModal]) == .extensionModal)
        #expect(
            MainWindowModalPolicy.topmost(in: [.extensionModal, .extensionWebviewModal])
                == .extensionWebviewModal
        )
    }
}

@Suite("Main window voice input policy")
struct MainWindowVoiceInputPolicyTests {
    @Test func opensLegacyPanelWithoutComposer() {
        #expect(MainWindowVoiceInputPolicy.target(composerVisible: false) == .legacyPanel)
    }

    @Test func activatesVoiceInVisibleComposer() {
        #expect(MainWindowVoiceInputPolicy.target(composerVisible: true) == .composer)
    }
}
