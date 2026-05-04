import AppKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "ViewportState")

@MainActor
final class ViewportState {
    let backingStore: TextBackingStore

    private(set) var viewportStartLine = 0
    private(set) var viewportEndLine = 0
    private(set) var documentVerticalPadding: CGFloat = 8

    let oracle: HeightOracle
    let heightMap: HeightMap

    var lineWrappingEnabled: Bool {
        get { oracle.lineWrapping }
        set {
            guard oracle.lineWrapping != newValue else { return }
            oracle.lineWrapping = newValue
            rebuildEstimates()
        }
    }

    var estimatedLineHeight: CGFloat { oracle.lineHeight }

    static let viewportBuffer = 500
    static let scrollHysteresis = 200

    var viewportLineCount: Int { viewportEndLine - viewportStartLine }

    var totalDocumentHeight: CGFloat {
        heightMap.totalHeight + documentVerticalPadding
    }

    init(backingStore: TextBackingStore) {
        self.backingStore = backingStore
        oracle = HeightOracle()
        heightMap = HeightMap(oracle: oracle)
        rebuildEstimates()
    }

    func updateEstimatedLineHeight(font: NSFont) {
        let lineHeight = ceil(NSLayoutManager().defaultLineHeight(for: font))
        oracle.updateLineHeight(lineHeight > 0 ? lineHeight : 16)
        oracle.updateCharWidth(estimatedCharWidth(for: font))
        rebuildEstimates()
    }

    func updateContainerWidth(_ width: CGFloat) {
        oracle.updateLineLength(containerWidth: width)
        rebuildEstimates()
    }

    func updateDocumentPadding(topInset: CGFloat, bottomInset: CGFloat, safetyPadding: CGFloat = 24) {
        documentVerticalPadding = topInset + bottomInset + safetyPadding
    }

    func resetMeasurements() {
        rebuildEstimates()
    }

    func recordMeasuredLineHeights(startLine: Int, lineHeights: [CGFloat]) {
        guard !lineHeights.isEmpty else { return }
        let charCounts = (0 ..< lineHeights.count).map { offset -> Int in
            let line = startLine + offset
            return charCount(forLine: line)
        }
        heightMap.applyMeasurements(startLine: startLine, lineHeights: lineHeights, lineCharCounts: charCounts)
    }

    func notifyLinesReplaced(start: Int, removingCount: Int, insertingLineCharCounts: [Int]) {
        heightMap.replaceLines(
            startLine: start,
            removingCount: removingCount,
            insertingLineCharCounts: insertingLineCharCounts
        )
    }

    func visibleLineRange(scrollY: CGFloat, visibleHeight: CGFloat) -> Range<Int> {
        guard backingStore.lineCount > 0 else { return 0 ..< 0 }
        let topY = max(0, scrollY)
        let bottomY = max(topY, scrollY + visibleHeight)
        let firstLocation = heightMap.lineAtY(topY)
        let lastLocation = heightMap.lineAtY(bottomY)
        let firstLine = max(0, min(firstLocation.line, backingStore.lineCount))
        var lastLine = min(backingStore.lineCount, lastLocation.line + 1)
        if lastLocation.topY >= bottomY, lastLine > firstLine {
            lastLine -= 1
        }
        return firstLine ..< max(firstLine, lastLine)
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
        heightMap.heightAbove(line: max(0, globalLine))
    }

    private func rebuildEstimates() {
        let charCounts = (0 ..< backingStore.lineCount).map { line in
            charCount(forLine: line)
        }
        heightMap.reset(lineCharCounts: charCounts)
    }

    private func charCount(forLine line: Int) -> Int {
        guard line >= 0, line < backingStore.lineCount else { return 0 }
        return (backingStore.line(at: line) as NSString).length
    }

    private func estimatedCharWidth(for font: NSFont) -> CGFloat {
        let attrs = [NSAttributedString.Key.font: font]
        let measured = ("M" as NSString).size(withAttributes: attrs).width
        return measured > 0 ? measured : 8
    }
}
