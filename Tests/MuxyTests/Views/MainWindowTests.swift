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
