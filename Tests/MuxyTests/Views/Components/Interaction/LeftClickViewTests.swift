import AppKit
import Testing

@testable import Muxy

@MainActor
@Suite("LeftClickView")
struct LeftClickViewTests {
    @Test("stays transparent while no primary press is being routed")
    func staysTransparentWithoutPrimaryPress() {
        let view = LeftClickNSView()
        view.frame = NSRect(x: 0, y: 0, width: 14, height: 14)

        #expect(view.hitTest(NSPoint(x: 7, y: 7)) == nil)
    }

    @Test("fires once the primary press is released inside the control")
    func firesWhenReleasedInsideControl() throws {
        var closes = 0
        let view = hostedView { closes += 1 }

        view.mouseDown(with: try event(.leftMouseDown, at: NSPoint(x: 17, y: 17)))
        view.mouseUp(with: try event(.leftMouseUp, at: NSPoint(x: 17, y: 17)))

        #expect(closes == 1)
    }

    @Test("ignores a press released outside the control")
    func ignoresReleaseOutsideControl() throws {
        var closes = 0
        let view = hostedView { closes += 1 }

        view.mouseDown(with: try event(.leftMouseDown, at: NSPoint(x: 17, y: 17)))
        view.mouseUp(with: try event(.leftMouseUp, at: NSPoint(x: 60, y: 60)))

        #expect(closes == 0)
    }

    @Test("fires for every press of a rapid click sequence")
    func firesForRapidClickSequence() throws {
        var closes = 0
        let view = hostedView { closes += 1 }

        for clickCount in 1 ... 3 {
            view.mouseDown(with: try event(.leftMouseDown, at: NSPoint(x: 17, y: 17), clickCount: clickCount))
            view.mouseUp(with: try event(.leftMouseUp, at: NSPoint(x: 17, y: 17), clickCount: clickCount))
        }

        #expect(closes == 3)
    }

    private func hostedView(action: @escaping () -> Void) -> LeftClickNSView {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        let view = LeftClickNSView()
        view.frame = NSRect(x: 10, y: 10, width: 14, height: 14)
        view.action = action
        window.contentView?.addSubview(view)
        return view
    }

    private func event(
        _ type: NSEvent.EventType,
        at location: NSPoint,
        clickCount: Int = 1
    ) throws -> NSEvent {
        try #require(NSEvent.mouseEvent(
            with: type,
            location: location,
            modifierFlags: [],
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            eventNumber: 0,
            clickCount: clickCount,
            pressure: 1
        ))
    }
}
