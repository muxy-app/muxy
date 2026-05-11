import AppKit
import Testing

@testable import Muxy

@Suite("ViewportState")
@MainActor
struct ViewportStateTests {
    private func makeViewport(lineCount: Int) -> ViewportState {
        let store = TextBackingStore()
        let text = (0 ..< lineCount).map { "line \($0)" }.joined(separator: "\n")
        store.loadFromText(text)
        return ViewportState(backingStore: store)
    }

    @Test("initial state has zero viewport and default line height")
    func initialState() {
        let vp = makeViewport(lineCount: 100)
        #expect(vp.viewportStartLine == 0)
        #expect(vp.viewportEndLine == 0)
        #expect(vp.estimatedLineHeight == 16)
        #expect(vp.viewportLineCount == 0)
    }

    @Test("totalDocumentHeight includes document padding")
    func totalDocumentHeight() {
        let vp = makeViewport(lineCount: 100)
        #expect(vp.totalDocumentHeight == 1608)
    }

    @Test("visibleLineRange at scroll 0")
    func visibleLineRangeTop() {
        let vp = makeViewport(lineCount: 100)
        let range = vp.visibleLineRange(scrollY: 0, visibleHeight: 160)
        #expect(range == 0 ..< 10)
    }

    @Test("visibleLineRange mid-scroll")
    func visibleLineRangeMid() {
        let vp = makeViewport(lineCount: 100)
        let range = vp.visibleLineRange(scrollY: 320, visibleHeight: 160)
        #expect(range.lowerBound == 20)
        #expect(range.upperBound == 30)
    }

    @Test("visibleLineRange clamps to lineCount")
    func visibleLineRangeClamp() {
        let vp = makeViewport(lineCount: 5)
        let range = vp.visibleLineRange(scrollY: 0, visibleHeight: 1600)
        #expect(range.upperBound == 5)
    }

    @Test("computeViewport adds buffer")
    func computeViewportBuffer() {
        let vp = makeViewport(lineCount: 2000)
        let range = vp.computeViewport(scrollY: 16000, visibleHeight: 160)
        let visible = vp.visibleLineRange(scrollY: 16000, visibleHeight: 160)
        let expectedStart = max(0, visible.lowerBound - ViewportState.viewportBuffer)
        let expectedEnd = min(2000, visible.upperBound + ViewportState.viewportBuffer)
        #expect(range.lowerBound == expectedStart)
        #expect(range.upperBound == expectedEnd)
    }

    @Test("computeViewport clamps to bounds")
    func computeViewportClamp() {
        let vp = makeViewport(lineCount: 100)
        let range = vp.computeViewport(scrollY: 0, visibleHeight: 160)
        #expect(range.lowerBound == 0)
        #expect(range.upperBound == 100)
    }

    @Test("shouldUpdateViewport returns true initially")
    func shouldUpdateInitially() {
        let vp = makeViewport(lineCount: 100)
        #expect(vp.shouldUpdateViewport(scrollY: 0, visibleHeight: 160))
    }

    @Test("shouldUpdateViewport returns false with adequate margin")
    func shouldUpdateFalseAdequateMargin() {
        let vp = makeViewport(lineCount: 2000)
        vp.applyViewport(0 ..< 1000)
        #expect(!vp.shouldUpdateViewport(scrollY: 8000, visibleHeight: 160))
    }

    @Test("shouldUpdateViewport returns true when margin below hysteresis")
    func shouldUpdateTrueLowMargin() {
        let vp = makeViewport(lineCount: 2000)
        vp.applyViewport(0 ..< 600)
        #expect(vp.shouldUpdateViewport(scrollY: 6400, visibleHeight: 160))
    }

    @Test("applyViewport sets start and end")
    func applyViewport() {
        let vp = makeViewport(lineCount: 100)
        vp.applyViewport(10 ..< 50)
        #expect(vp.viewportStartLine == 10)
        #expect(vp.viewportEndLine == 50)
        #expect(vp.viewportLineCount == 40)
    }

    @Test("viewportText returns correct content")
    func viewportText() {
        let vp = makeViewport(lineCount: 10)
        vp.applyViewport(2 ..< 5)
        let text = vp.viewportText()
        #expect(text == "line 2\nline 3\nline 4")
    }

    @Test("viewportYOffset is start * lineHeight")
    func viewportYOffset() {
        let vp = makeViewport(lineCount: 100)
        vp.applyViewport(10 ..< 50)
        #expect(vp.viewportYOffset() == 160)
    }

    @Test("backingStoreLine adds viewport offset")
    func backingStoreLine() {
        let vp = makeViewport(lineCount: 100)
        vp.applyViewport(20 ..< 50)
        #expect(vp.backingStoreLine(forViewportLine: 0) == 20)
        #expect(vp.backingStoreLine(forViewportLine: 5) == 25)
    }

    @Test("viewportLine returns local index for in-viewport line")
    func viewportLineInRange() {
        let vp = makeViewport(lineCount: 100)
        vp.applyViewport(20 ..< 50)
        #expect(vp.viewportLine(forBackingStoreLine: 25) == 5)
    }

    @Test("viewportLine returns nil for out-of-viewport line")
    func viewportLineOutOfRange() {
        let vp = makeViewport(lineCount: 100)
        vp.applyViewport(20 ..< 50)
        #expect(vp.viewportLine(forBackingStoreLine: 10) == nil)
        #expect(vp.viewportLine(forBackingStoreLine: 50) == nil)
    }

    @Test("isLineInViewport returns correct values")
    func isLineInViewport() {
        let vp = makeViewport(lineCount: 100)
        vp.applyViewport(20 ..< 50)
        #expect(vp.isLineInViewport(25))
        #expect(!vp.isLineInViewport(10))
        #expect(!vp.isLineInViewport(50))
    }

    @Test("scrollY for line is line * lineHeight")
    func scrollYForLine() {
        let vp = makeViewport(lineCount: 100)
        #expect(vp.scrollY(forLine: 10) == 160)
        #expect(vp.scrollY(forLine: 0) == 0)
    }

    @Test("updateEstimatedLineHeight scales by multiplier")
    func lineHeightMultiplierScales() {
        let vp = makeViewport(lineCount: 10)
        let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        vp.updateEstimatedLineHeight(font: font, lineHeightMultiplier: 1.0)
        let base = vp.estimatedLineHeight

        vp.updateEstimatedLineHeight(font: font, lineHeightMultiplier: 2.0)
        let doubled = vp.estimatedLineHeight

        #expect(doubled >= base * 2 - 1)
        #expect(doubled <= base * 2 + 1)
    }

    @Test("multiplier scales totalDocumentHeight")
    func lineHeightMultiplierAffectsTotalHeight() {
        let vp = makeViewport(lineCount: 100)
        let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        vp.updateEstimatedLineHeight(font: font, lineHeightMultiplier: 1.0)
        let baseHeight = vp.totalDocumentHeight

        vp.updateEstimatedLineHeight(font: font, lineHeightMultiplier: 1.5)
        let taller = vp.totalDocumentHeight

        #expect(taller > baseHeight)
    }
}
