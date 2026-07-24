import AppKit
import Testing

@testable import Muxy

@Suite("Quick terminal geometry")
struct QuickTerminalGeometryTests {
    @Test("uses the maximum size at the top center of a large display")
    func maximumSizeAtTopCenter() {
        let display = NSRect(x: 100, y: 50, width: 1_440, height: 900)

        let frame = QuickTerminalGeometry.frame(in: display)

        #expect(frame == NSRect(x: 460, y: 520, width: 720, height: 430))
    }

    @Test("clamps both dimensions to a small display")
    func clampsToSmallDisplay() {
        let display = NSRect(x: -500, y: -200, width: 500, height: 320)

        let frame = QuickTerminalGeometry.frame(in: display)

        #expect(frame == display)
    }

    @Test("uses the configured size at the top center")
    func configuredSizeAtTopCenter() {
        let display = NSRect(x: 100, y: 50, width: 1_440, height: 900)

        let frame = QuickTerminalGeometry.frame(
            in: display,
            preferredSize: NSSize(width: 960, height: 600)
        )

        #expect(frame == NSRect(x: 340, y: 350, width: 960, height: 600))
    }

    @Test("empty display produces an empty frame")
    func emptyDisplay() {
        #expect(QuickTerminalGeometry.frame(in: .zero) == .zero)
    }

    @Test("uses the physical screen top while respecting horizontal visible bounds")
    func physicalTopAndVisibleBounds() {
        let screen = NSRect(x: 0, y: 0, width: 1_440, height: 900)
        let visible = NSRect(x: 80, y: 0, width: 1_360, height: 875)

        let frame = QuickTerminalGeometry.frame(screenFrame: screen, visibleFrame: visible)

        #expect(frame == NSRect(x: 360, y: 470, width: 720, height: 430))
    }
}

@Suite("Quick terminal active screen resolution")
struct QuickTerminalScreenResolverTests {
    private let screens = [
        NSRect(x: -1_280, y: 0, width: 1_280, height: 800),
        NSRect(x: 0, y: 0, width: 1_440, height: 900),
    ]

    @Test("mouse display wins over window fallbacks")
    func mouseDisplayWins() {
        let index = QuickTerminalScreenResolver.preferredScreenIndex(
            mouseLocation: NSPoint(x: -600, y: 400),
            screenFrames: screens,
            keyScreenIndex: 1,
            mainWindowScreenIndex: 1,
            mainScreenIndex: 1
        )

        #expect(index == 0)
    }

    @Test("uses key then main window then main display fallbacks")
    func orderedFallbacks() {
        let outside = NSPoint(x: 5_000, y: 5_000)

        #expect(QuickTerminalScreenResolver.preferredScreenIndex(
            mouseLocation: outside,
            screenFrames: screens,
            keyScreenIndex: 1,
            mainWindowScreenIndex: 0,
            mainScreenIndex: 0
        ) == 1)
        #expect(QuickTerminalScreenResolver.preferredScreenIndex(
            mouseLocation: outside,
            screenFrames: screens,
            keyScreenIndex: nil,
            mainWindowScreenIndex: 0,
            mainScreenIndex: 1
        ) == 0)
        #expect(QuickTerminalScreenResolver.preferredScreenIndex(
            mouseLocation: outside,
            screenFrames: screens,
            keyScreenIndex: nil,
            mainWindowScreenIndex: nil,
            mainScreenIndex: 1
        ) == 1)
    }

    @Test("falls back to the first display and handles no displays")
    func finalFallbacks() {
        let outside = NSPoint(x: 5_000, y: 5_000)

        #expect(QuickTerminalScreenResolver.preferredScreenIndex(
            mouseLocation: outside,
            screenFrames: screens,
            keyScreenIndex: nil,
            mainWindowScreenIndex: nil,
            mainScreenIndex: nil
        ) == 0)
        #expect(QuickTerminalScreenResolver.preferredScreenIndex(
            mouseLocation: outside,
            screenFrames: [],
            keyScreenIndex: nil,
            mainWindowScreenIndex: nil,
            mainScreenIndex: nil
        ) == nil)
    }
}
