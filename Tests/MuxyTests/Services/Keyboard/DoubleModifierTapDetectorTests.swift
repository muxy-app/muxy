import Foundation
import Testing

@testable import Muxy

@Suite("DoubleModifierTapDetector")
struct DoubleModifierTapDetectorTests {
    @Test("two complete modifier taps trigger once")
    func twoCompleteTapsTrigger() {
        var detector = DoubleModifierTapDetector()

        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: false, otherModifierPressed: false, timestamp: 1.1))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.2))
        expectTrigger(&detector, .modifierChange(modifierPressed: false, otherModifierPressed: false, timestamp: 1.3))
    }

    @Test("repeated press events do not replace releases")
    func repeatedPressEventsDoNotTrigger() {
        var detector = DoubleModifierTapDetector()

        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.1))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.2))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: false, otherModifierPressed: false, timestamp: 1.3))
    }

    @Test("configured interval controls recognition")
    func configuredIntervalControlsRecognition() {
        var detector = DoubleModifierTapDetector(configuration: .init(maximumTapDuration: 0.3, maximumInterval: 0.5))

        tap(&detector, startingAt: 1.0)
        expectTap(&detector, startingAt: 1.45, triggers: true)
    }

    @Test("tap outside configured interval starts a new sequence")
    func outsideIntervalStartsNewSequence() {
        var detector = DoubleModifierTapDetector(configuration: .init(maximumTapDuration: 0.3, maximumInterval: 0.2))

        tap(&detector, startingAt: 1.0)
        expectTap(&detector, startingAt: 1.4, triggers: false)
        expectTap(&detector, startingAt: 1.55, triggers: true)
    }

    @Test("key use blocks the held modifier")
    func keyUseBlocksHeldModifier() {
        var detector = DoubleModifierTapDetector()

        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .keyDown(modifierPressed: true, timestamp: 1.1))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: false, otherModifierPressed: false, timestamp: 1.2))
        expectTap(&detector, startingAt: 1.3, triggers: false)
        expectTap(&detector, startingAt: 1.5, triggers: true)
    }

    @Test("pointer use blocks the held modifier")
    func pointerUseBlocksHeldModifier() {
        var detector = DoubleModifierTapDetector()

        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .pointerDown(modifierPressed: true, timestamp: 1.1))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: false, otherModifierPressed: false, timestamp: 1.2))
        expectTap(&detector, startingAt: 1.3, triggers: false)
        expectTap(&detector, startingAt: 1.5, triggers: true)
    }

    @Test("another modifier resets and blocks the sequence")
    func otherModifierBlocksSequence() {
        var detector = DoubleModifierTapDetector()

        tap(&detector, startingAt: 1.0)
        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: true, timestamp: 1.2))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.25))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: false, otherModifierPressed: false, timestamp: 1.3))
        expectTap(&detector, startingAt: 1.4, triggers: false)
        expectTap(&detector, startingAt: 1.6, triggers: true)
    }

    @Test("long press does not count as a tap")
    func longPressDoesNotCount() {
        var detector = DoubleModifierTapDetector(configuration: .init(maximumTapDuration: 0.2, maximumInterval: 0.5))

        expectNoTrigger(&detector, .modifierChange(modifierPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .modifierChange(modifierPressed: false, otherModifierPressed: false, timestamp: 1.3))
        expectTap(&detector, startingAt: 1.4, triggers: false)
        expectTap(&detector, startingAt: 1.6, triggers: true)
    }

    @Test("out-of-order timestamps reset safely")
    func outOfOrderTimestampsReset() {
        var detector = DoubleModifierTapDetector()

        tap(&detector, startingAt: 2.0)
        expectTap(&detector, startingAt: 1.0, triggers: false)
        expectTap(&detector, startingAt: 1.2, triggers: true)
    }

    @discardableResult
    private func tap(_ detector: inout DoubleModifierTapDetector, startingAt timestamp: TimeInterval) -> Bool {
        _ = detector.process(.modifierChange(
            modifierPressed: true,
            otherModifierPressed: false,
            timestamp: timestamp
        ))
        return detector.process(.modifierChange(
            modifierPressed: false,
            otherModifierPressed: false,
            timestamp: timestamp + 0.1
        ))
    }

    private func expectNoTrigger(
        _ detector: inout DoubleModifierTapDetector,
        _ input: DoubleModifierTapDetector.Input
    ) {
        let triggered = detector.process(input)
        #expect(!triggered)
    }

    private func expectTrigger(
        _ detector: inout DoubleModifierTapDetector,
        _ input: DoubleModifierTapDetector.Input
    ) {
        let triggered = detector.process(input)
        #expect(triggered)
    }

    private func expectTap(
        _ detector: inout DoubleModifierTapDetector,
        startingAt timestamp: TimeInterval,
        triggers: Bool
    ) {
        #expect(tap(&detector, startingAt: timestamp) == triggers)
    }
}
