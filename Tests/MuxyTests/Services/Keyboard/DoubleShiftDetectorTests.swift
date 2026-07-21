import Foundation
import Testing

@testable import Muxy

@Suite("DoubleShiftDetector")
struct DoubleShiftDetectorTests {
    @Test("persistent Caps Lock does not count as a held modifier")
    func persistentCapsLockDoesNotBlock() {
        var modifierState = DoubleShiftModifierState()

        let initial = modifierState.otherModifierPressed(
            conventionalModifierPressed: false,
            capsLockEnabled: true
        )
        let persistent = modifierState.otherModifierPressed(
            conventionalModifierPressed: false,
            capsLockEnabled: true
        )

        #expect(!initial)
        #expect(!persistent)
    }

    @Test("Caps Lock transitions and conventional modifiers reset the sequence")
    func modifierStateDetectsInterveningModifiers() {
        var modifierState = DoubleShiftModifierState()
        _ = modifierState.otherModifierPressed(
            conventionalModifierPressed: false,
            capsLockEnabled: false
        )

        let capsLockTransition = modifierState.otherModifierPressed(
            conventionalModifierPressed: false,
            capsLockEnabled: true
        )
        let conventionalModifier = modifierState.otherModifierPressed(
            conventionalModifierPressed: true,
            capsLockEnabled: true
        )

        #expect(capsLockTransition)
        #expect(conventionalModifier)
    }

    @Test("two complete Shift taps trigger once")
    func twoCompleteTapsTrigger() {
        var detector = DoubleShiftDetector()

        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: false, otherModifierPressed: false, timestamp: 1.1))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.2))
        expectTrigger(&detector, .modifierChange(shiftPressed: false, otherModifierPressed: false, timestamp: 1.3))
    }

    @Test("repeated press events never substitute for releases")
    func completeTapsRequired() {
        var detector = DoubleShiftDetector()

        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.1))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.2))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: false, otherModifierPressed: false, timestamp: 1.3))
    }

    @Test("an intervening key resets the sequence")
    func interveningKeyResets() {
        var detector = DoubleShiftDetector()

        tapShift(detector: &detector, startingAt: 1.0)
        expectNoTrigger(&detector, .keyDown(shiftPressed: false, timestamp: 1.2))
        expectTap(&detector, startingAt: 1.3, triggers: false)
        expectTap(&detector, startingAt: 1.5, triggers: true)
    }

    @Test("an intervening pointer press resets and blocks a held Shift")
    func interveningPointerPressResets() {
        var detector = DoubleShiftDetector()

        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .pointerDown(shiftPressed: true, timestamp: 1.1))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: false, otherModifierPressed: false, timestamp: 1.2))
        expectTap(&detector, startingAt: 1.3, triggers: false)
        expectTap(&detector, startingAt: 1.5, triggers: true)
    }

    @Test("another modifier blocks Shift until it is released")
    func otherModifierResetsAndBlocksHeldShift() {
        var detector = DoubleShiftDetector()

        tapShift(detector: &detector, startingAt: 1.0)
        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: true, timestamp: 1.2))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.25))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: false, otherModifierPressed: false, timestamp: 1.3))
        expectTap(&detector, startingAt: 1.4, triggers: false)
        expectTap(&detector, startingAt: 1.6, triggers: true)
    }

    @Test("timeout starts a fresh sequence")
    func timeoutResets() {
        var detector = DoubleShiftDetector(configuration: .init(maximumTapDuration: 0.2, maximumInterval: 0.3))

        tapShift(detector: &detector, startingAt: 1.0)
        expectTap(&detector, startingAt: 1.5, triggers: false)
        expectTap(&detector, startingAt: 1.7, triggers: true)
    }

    @Test("holding either Shift tap prevents a trigger")
    func heldShiftResets() {
        var detector = DoubleShiftDetector(configuration: .init(maximumTapDuration: 0.2, maximumInterval: 0.5))

        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.0))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: false, otherModifierPressed: false, timestamp: 1.3))
        expectTap(&detector, startingAt: 1.4, triggers: false)
        expectNoTrigger(&detector, .modifierChange(shiftPressed: true, otherModifierPressed: false, timestamp: 1.6))
        expectNoTrigger(&detector, .modifierChange(shiftPressed: false, otherModifierPressed: false, timestamp: 1.9))
    }

    @Test("out-of-order timestamps reset safely")
    func outOfOrderTimestampsReset() {
        var detector = DoubleShiftDetector()

        tapShift(detector: &detector, startingAt: 2.0)
        expectTap(&detector, startingAt: 1.0, triggers: false)
        expectTap(&detector, startingAt: 1.2, triggers: true)
    }

    @discardableResult
    private func tapShift(detector: inout DoubleShiftDetector, startingAt timestamp: TimeInterval) -> Bool {
        _ = detector.process(.modifierChange(
            shiftPressed: true,
            otherModifierPressed: false,
            timestamp: timestamp
        ))
        return detector.process(.modifierChange(
            shiftPressed: false,
            otherModifierPressed: false,
            timestamp: timestamp + 0.1
        ))
    }

    private func expectNoTrigger(_ detector: inout DoubleShiftDetector, _ input: DoubleShiftDetector.Input) {
        let triggered = detector.process(input)
        #expect(!triggered)
    }

    private func expectTrigger(_ detector: inout DoubleShiftDetector, _ input: DoubleShiftDetector.Input) {
        let triggered = detector.process(input)
        #expect(triggered)
    }

    private func expectTap(
        _ detector: inout DoubleShiftDetector,
        startingAt timestamp: TimeInterval,
        triggers: Bool
    ) {
        let triggered = tapShift(detector: &detector, startingAt: timestamp)
        #expect(triggered == triggers)
    }
}
