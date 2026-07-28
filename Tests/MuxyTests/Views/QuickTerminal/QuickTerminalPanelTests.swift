import AppKit
import Testing

@testable import Muxy

@MainActor
@Suite("Quick terminal panel")
struct QuickTerminalPanelTests {
    @Test("routes key-down events to the handler instead of dismissing")
    func routesKeyDownToHandler() throws {
        let panel = QuickTerminalPanel(contentRect: .zero)
        var routed: [UInt16] = []
        panel.onKeyDown = { event in
            routed.append(event.keyCode)
            return true
        }

        let escape = try #require(NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: [],
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: "\u{1b}",
            charactersIgnoringModifiers: "\u{1b}",
            isARepeat: false,
            keyCode: 53
        ))
        panel.sendEvent(escape)

        #expect(routed == [53])
    }
}
