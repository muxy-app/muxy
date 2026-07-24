import AppKit
import Carbon.HIToolbox
import Testing

@testable import Muxy

@MainActor
@Suite("OverlayEscapeKeyMonitor")
struct OverlayEscapeKeyMonitorTests {
    @Test("consumes escape and invokes action")
    func consumesEscape() throws {
        var fired = false
        let event = try #require(makeKeyEvent(keyCode: UInt16(kVK_Escape)))

        let result = OverlayEscapeKeyHandler.handle(event) { fired = true }

        #expect(result == nil)
        #expect(fired)
    }

    @Test("passes through non-escape keys")
    func passesThroughOtherKeys() throws {
        var fired = false
        let event = try #require(makeKeyEvent(keyCode: UInt16(kVK_Return)))

        let result = OverlayEscapeKeyHandler.handle(event) { fired = true }

        #expect(result === event)
        #expect(!fired)
    }

    private func makeKeyEvent(keyCode: UInt16) -> NSEvent? {
        NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: [],
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: "",
            charactersIgnoringModifiers: "",
            isARepeat: false,
            keyCode: keyCode
        )
    }
}
