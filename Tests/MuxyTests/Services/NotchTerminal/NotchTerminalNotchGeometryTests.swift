import AppKit
import Testing

@testable import Muxy

@Suite("Notch terminal notch geometry")
struct NotchTerminalNotchGeometryTests {
    private let macBookFrame = NSRect(x: 0, y: 0, width: 1_512, height: 982)

    @Test("returns nil without both auxiliary widths")
    func requiresBothAuxiliaryWidths() {
        #expect(NotchTerminalNotchGeometry.notchRect(
            screenFrame: macBookFrame,
            safeAreaTop: 37,
            leftAuxiliaryWidth: nil,
            rightAuxiliaryWidth: 620
        ) == nil)
        #expect(NotchTerminalNotchGeometry.notchRect(
            screenFrame: macBookFrame,
            safeAreaTop: 37,
            leftAuxiliaryWidth: 620,
            rightAuxiliaryWidth: nil
        ) == nil)
    }

    @Test("returns nil without a safe-area inset")
    func requiresSafeAreaTop() {
        #expect(NotchTerminalNotchGeometry.notchRect(
            screenFrame: macBookFrame,
            safeAreaTop: 0,
            leftAuxiliaryWidth: 620,
            rightAuxiliaryWidth: 620
        ) == nil)
    }

    @Test("returns nil when the notch width is not positive")
    func requiresPositiveNotchWidth() {
        #expect(NotchTerminalNotchGeometry.notchRect(
            screenFrame: macBookFrame,
            safeAreaTop: 37,
            leftAuxiliaryWidth: 760,
            rightAuxiliaryWidth: 760
        ) == nil)
    }

    @Test("computes the notch rect at the top center")
    func computesNotchRect() {
        #expect(NotchTerminalNotchGeometry.notchRect(
            screenFrame: macBookFrame,
            safeAreaTop: 37,
            leftAuxiliaryWidth: 620,
            rightAuxiliaryWidth: 620
        ) == NSRect(x: 620, y: 945, width: 272, height: 37))
    }

    @Test("honors a non-zero screen origin")
    func honorsScreenOrigin() {
        #expect(NotchTerminalNotchGeometry.notchRect(
            screenFrame: NSRect(x: 100, y: 50, width: 1_512, height: 982),
            safeAreaTop: 37,
            leftAuxiliaryWidth: 620,
            rightAuxiliaryWidth: 620
        ) == NSRect(x: 720, y: 995, width: 272, height: 37))
    }

    @Test("maps the notch rect into panel-local coordinates")
    func mapsCollapsedRect() {
        let notch = NSRect(x: 620, y: 945, width: 272, height: 37)
        let panel = NSRect(x: 396, y: 552, width: 720, height: 430)

        #expect(NotchTerminalNotchGeometry.collapsedRect(notchRect: notch, panelFrame: panel)
            == NSRect(x: 224, y: 393, width: 272, height: 37))
    }

    @Test("keeps the collapsed rect anchored when the panel is horizontally clamped")
    func mapsCollapsedRectForClampedPanel() {
        let notch = NSRect(x: 620, y: 945, width: 272, height: 37)
        let panel = NSRect(x: 80, y: 552, width: 1_360, height: 430)

        #expect(NotchTerminalNotchGeometry.collapsedRect(notchRect: notch, panelFrame: panel)
            == NSRect(x: 540, y: 393, width: 272, height: 37))
    }
}
