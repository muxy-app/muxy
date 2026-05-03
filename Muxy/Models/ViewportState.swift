import AppKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "ViewportState")

@MainActor
final class ViewportState {
    let backingStore: TextBackingStore

    private(set) var viewportStartLine = 0
    private(set) var viewportEndLine = 0
    private(set) var estimatedLineHeight: CGFloat = 16
    private(set) var documentVerticalPadding: CGFloat = 8
    private(set) var wrappedHeights: WrappedLineHeights
    var lineWrappingEnabled: Bool = false

    static let viewportBuffer = 500
    static let scrollHysteresis = 200

    var viewportLineCount: Int { viewportEndLine - viewportStartLine }

    var totalDocumentHeight: CGFloat {
        if lineWrappingEnabled {
            wrappedHeights.resize(to: backingStore.lineCount)
            return CGFloat(wrappedHeights.totalFragments) * estimatedLineHeight + documentVerticalPadding
        }
        return CGFloat(backingStore.lineCount) * estimatedLineHeight + documentVerticalPadding
    }

    init(backingStore: TextBackingStore) {
        self.backingStore = backingStore
        wrappedHeights = WrappedLineHeights(lineCount: backingStore.lineCount)
    }

    func updateEstimatedLineHeight(font: NSFont) {
        estimatedLineHeight = ceil(NSLayoutManager().defaultLineHeight(for: font))
        if estimatedLineHeight < 1 {
            estimatedLineHeight = 16
        }
    }

    func updateDocumentPadding(topInset: CGFloat, bottomInset: CGFloat, safetyPadding: CGFloat = 24) {
        documentVerticalPadding = topInset + bottomInset + safetyPadding
    }

    func resetWrappedHeights() {
        wrappedHeights.resize(to: backingStore.lineCount)
        wrappedHeights.resetAllToBaseline()
    }

    func recordWrappedFragmentCount(_ count: Int, atLine line: Int) {
        wrappedHeights.resize(to: backingStore.lineCount)
        wrappedHeights.setFragmentCount(count, at: line)
    }

    func notifyLinesReplaced(start: Int, removingCount: Int, insertingCount: Int) {
        guard lineWrappingEnabled else { return }
        wrappedHeights.replaceLines(
            start: start,
            removingCount: removingCount,
            insertingCount: insertingCount
        )
    }

    func visibleLineRange(scrollY: CGFloat, visibleHeight: CGFloat) -> Range<Int> {
        let unit = estimatedLineHeight
        if lineWrappingEnabled, unit > 0 {
            wrappedHeights.resize(to: backingStore.lineCount)
            let firstFragment = max(0, Int(floor(scrollY / unit)))
            let lastFragment = max(firstFragment, Int(ceil((scrollY + visibleHeight) / unit)))
            let firstLine = wrappedHeights.line(forFragmentOffset: firstFragment)
            let lastLine = min(backingStore.lineCount, wrappedHeights.line(forFragmentOffset: lastFragment) + 1)
            return firstLine ..< max(firstLine, lastLine)
        }
        let firstVisible = max(0, Int(floor(scrollY / unit)))
        let lastVisible = min(
            backingStore.lineCount,
            Int(ceil((scrollY + visibleHeight) / unit))
        )
        return firstVisible ..< max(firstVisible, lastVisible)
    }

    func computeViewport(scrollY: CGFloat, visibleHeight: CGFloat) -> Range<Int> {
        let visible = visibleLineRange(scrollY: scrollY, visibleHeight: visibleHeight)
        let start = max(0, visible.lowerBound - Self.viewportBuffer)
        let end = min(backingStore.lineCount, visible.upperBound + Self.viewportBuffer)
        return start ..< max(start, end)
    }

    func shouldUpdateViewport(scrollY: CGFloat, visibleHeight: CGFloat) -> Bool {
        let visible = visibleLineRange(scrollY: scrollY, visibleHeight: visibleHeight)
        guard viewportStartLine < viewportEndLine else { return true }

        let topMargin = visible.lowerBound - viewportStartLine
        let bottomMargin = viewportEndLine - visible.upperBound
        return topMargin < Self.scrollHysteresis || bottomMargin < Self.scrollHysteresis
    }

    func applyViewport(_ range: Range<Int>) {
        viewportStartLine = range.lowerBound
        viewportEndLine = range.upperBound
    }

    func viewportText() -> String {
        backingStore.textForRange(viewportStartLine ..< viewportEndLine)
    }

    func viewportYOffset() -> CGFloat {
        scrollY(forLine: viewportStartLine)
    }

    func backingStoreLine(forViewportLine localLine: Int) -> Int {
        viewportStartLine + localLine
    }

    func viewportLine(forBackingStoreLine globalLine: Int) -> Int? {
        guard globalLine >= viewportStartLine, globalLine < viewportEndLine else { return nil }
        return globalLine - viewportStartLine
    }

    func isLineInViewport(_ globalLine: Int) -> Bool {
        globalLine >= viewportStartLine && globalLine < viewportEndLine
    }

    func scrollY(forLine globalLine: Int) -> CGFloat {
        if lineWrappingEnabled {
            wrappedHeights.resize(to: backingStore.lineCount)
            guard globalLine > 0 else { return 0 }
            let fragmentsBefore = wrappedHeights.prefixFragments(throughLine: globalLine - 1)
            return CGFloat(fragmentsBefore) * estimatedLineHeight
        }
        return CGFloat(globalLine) * estimatedLineHeight
    }
}
