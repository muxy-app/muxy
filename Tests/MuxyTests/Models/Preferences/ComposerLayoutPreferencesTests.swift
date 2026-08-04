import CoreGraphics
import Testing

@testable import Muxy

@Suite("Composer layout preferences")
struct ComposerLayoutPreferencesTests {
    private let window = CGSize(width: 1400, height: 900)

    @Test("stored size is used when it fits the window")
    func usesStoredSize() {
        let size = ComposerLayoutPreferences.size(
            width: 800,
            height: 500,
            isExpanded: false,
            available: window
        )

        #expect(size == CGSize(width: 800, height: 500))
    }

    @Test("size falls back to defaults for unusable stored values")
    func fallsBackToDefaults() {
        let size = ComposerLayoutPreferences.size(
            width: 0,
            height: .nan,
            isExpanded: false,
            available: window
        )

        #expect(size.width == CGFloat(ComposerLayoutPreferences.defaultWidth))
        #expect(size.height == CGFloat(ComposerLayoutPreferences.defaultHeight))
    }

    @Test("size never drops below the minimums")
    func clampsToMinimums() {
        let size = ComposerLayoutPreferences.size(
            width: 10,
            height: 10,
            isExpanded: false,
            available: window
        )

        #expect(size.width == ComposerLayoutPreferences.minWidth)
        #expect(size.height == ComposerLayoutPreferences.minHeight)
    }

    @Test("size is clamped to the window without changing the stored preference")
    func clampsToAvailableSpace() {
        let available = CGSize(width: 600, height: 400)
        let margin = ComposerLayoutPreferences.edgeMargin * 2

        let size = ComposerLayoutPreferences.size(
            width: 5000,
            height: 5000,
            isExpanded: false,
            available: available
        )

        #expect(size == CGSize(width: available.width - margin, height: available.height - margin))
    }

    @Test("a window smaller than the minimum wins over the minimum")
    func availableSpaceWinsOverMinimums() {
        let available = CGSize(width: 200, height: 120)

        let size = ComposerLayoutPreferences.size(
            width: ComposerLayoutPreferences.defaultWidth,
            height: ComposerLayoutPreferences.defaultHeight,
            isExpanded: false,
            available: available
        )

        #expect(size.width < ComposerLayoutPreferences.minWidth)
        #expect(size.height < ComposerLayoutPreferences.minHeight)
        #expect(size.width == available.width - ComposerLayoutPreferences.edgeMargin * 2)
    }

    @Test("expanded size stops at the readable preset on a large window")
    func expandedStopsAtThePreset() {
        let size = ComposerLayoutPreferences.size(
            width: ComposerLayoutPreferences.defaultWidth,
            height: ComposerLayoutPreferences.defaultHeight,
            isExpanded: true,
            available: window
        )

        #expect(size == CGSize(
            width: ComposerLayoutPreferences.expandedWidth,
            height: ComposerLayoutPreferences.expandedHeight
        ))
        #expect(size.width < window.width)
        #expect(size.height < window.height)
    }

    @Test("expanded size still fits a window smaller than the preset")
    func expandedFitsSmallWindow() {
        let available = CGSize(width: 600, height: 400)
        let margin = ComposerLayoutPreferences.edgeMargin * 2

        let size = ComposerLayoutPreferences.size(
            width: ComposerLayoutPreferences.defaultWidth,
            height: ComposerLayoutPreferences.defaultHeight,
            isExpanded: true,
            available: available
        )

        #expect(size == CGSize(width: available.width - margin, height: available.height - margin))
    }

    @Test("expanded size is larger than the default size")
    func expandedIsLargerThanDefault() {
        #expect(ComposerLayoutPreferences.expandedWidth > CGFloat(ComposerLayoutPreferences.defaultWidth))
        #expect(ComposerLayoutPreferences.expandedHeight > CGFloat(ComposerLayoutPreferences.defaultHeight))
    }

    @Test("an unusable window falls back to a finite size")
    func handlesUnusableWindow() {
        let size = ComposerLayoutPreferences.size(
            width: 5000,
            height: 5000,
            isExpanded: true,
            available: CGSize(width: 0, height: CGFloat.nan)
        )

        #expect(size.width == CGFloat(ComposerLayoutPreferences.defaultWidth))
        #expect(size.height == CGFloat(ComposerLayoutPreferences.defaultHeight))
    }

    @Test("dragging a trailing edge grows the centered box at twice the translation")
    func trailingEdgeGrowsSymmetrically() {
        let length = ComposerLayoutPreferences.resizedLength(
            anchor: 400,
            translation: 50,
            isLeadingEdge: false
        )

        #expect(length == 500)
    }

    @Test("dragging a leading edge outward grows the centered box")
    func leadingEdgeGrowsOutward() {
        let length = ComposerLayoutPreferences.resizedLength(
            anchor: 400,
            translation: -50,
            isLeadingEdge: true
        )

        #expect(length == 500)
    }

    @Test("dragging a leading edge inward shrinks the centered box")
    func leadingEdgeShrinksInward() {
        let length = ComposerLayoutPreferences.resizedLength(
            anchor: 400,
            translation: 50,
            isLeadingEdge: true
        )

        #expect(length == 300)
    }

    @Test("an unusable translation keeps the anchor")
    func ignoresUnusableTranslation() {
        let length = ComposerLayoutPreferences.resizedLength(
            anchor: 400,
            translation: .nan,
            isLeadingEdge: false
        )

        #expect(length == 400)
    }

    @Test("dragged lengths are clamped to the window and the minimums")
    func clampsDraggedLengths() {
        let available = CGSize(width: 700, height: 500)

        #expect(ComposerLayoutPreferences.clampedWidth(50, available: available) == ComposerLayoutPreferences.minWidth)
        #expect(ComposerLayoutPreferences.clampedHeight(50, available: available) == ComposerLayoutPreferences.minHeight)
        #expect(ComposerLayoutPreferences.clampedWidth(5000, available: available) == 660)
        #expect(ComposerLayoutPreferences.clampedHeight(5000, available: available) == 460)
    }
}
