import CoreGraphics
import Testing

@testable import Muxy

@Suite("TabStripLayout")
struct TabStripLayoutTests {
    private static let buttonWidth: CGFloat = 28

    private func layout(width: CGFloat, tabs: Int, maxTabWidth: CGFloat? = 200) -> TabStripLayout {
        TabStripLayout(
            availableWidth: width,
            tabCount: tabs,
            maxTabWidth: maxTabWidth,
            newTabButtonWidth: Self.buttonWidth
        )
    }

    @Test("reserves the new tab button width so the row fills the strip exactly")
    func reservesButtonWidth() {
        let availableWidth: CGFloat = 400
        let tabCount: CGFloat = 5
        let expectedWidth = (availableWidth - Self.buttonWidth) / tabCount
        let result = layout(width: availableWidth, tabs: 5)
        #expect(!result.pinsNewTabButton)
        #expect(result.perTabWidth == expectedWidth)
        #expect(result.tabRowWidth == availableWidth)
        #expect(tabCount * result.perTabWidth + Self.buttonWidth == availableWidth)
    }

    @Test("caps tab width at the configured maximum and keeps the button inline")
    func capsAtMaxTabWidth() {
        let result = layout(width: 1400, tabs: 2)
        #expect(result.perTabWidth == 200)
        #expect(!result.pinsNewTabButton)
        #expect(result.tabRowWidth == 1400)
    }

    @Test("falls back to the maximum tab width before the geometry is measured")
    func usesFallbackWidthWhenUnmeasured() {
        let result = layout(width: 0, tabs: 3)
        #expect(result.perTabWidth == TabStripLayout.maxTabWidth)
        #expect(!result.pinsNewTabButton)
    }

    @Test("pins the new tab button once tabs hit the minimum width")
    func pinsButtonOnOverflow() {
        let result = layout(width: 400, tabs: 9)
        #expect(result.pinsNewTabButton)
        #expect(result.perTabWidth == TabStripLayout.minTabWidth)
        #expect(result.tabRowWidth == 400 - Self.buttonWidth)
    }

    @Test("keeps the button inline at the exact overflow boundary")
    func keepsButtonInlineAtBoundary() {
        let exactWidth = Self.buttonWidth + TabStripLayout.minTabWidth * 9
        let fitting = layout(width: exactWidth, tabs: 9)
        #expect(!fitting.pinsNewTabButton)
        #expect(fitting.perTabWidth == TabStripLayout.minTabWidth)

        let overflowing = layout(width: exactWidth - 1, tabs: 9)
        #expect(overflowing.pinsNewTabButton)
        #expect(overflowing.perTabWidth == TabStripLayout.minTabWidth)
    }
}
