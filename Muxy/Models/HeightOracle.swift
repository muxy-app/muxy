import Foundation

@MainActor
final class HeightOracle {
    private(set) var lineHeight: CGFloat = 16
    private(set) var charWidth: CGFloat = 8
    private(set) var lineLength: CGFloat = 30
    var lineWrapping: Bool = false

    func updateLineHeight(_ height: CGFloat) {
        guard height > 0 else { return }
        lineHeight = height
    }

    func updateCharWidth(_ width: CGFloat) {
        guard width > 0 else { return }
        charWidth = width
    }

    @discardableResult
    func updateLineLength(containerWidth: CGFloat) -> Bool {
        guard containerWidth > 0, charWidth > 0 else { return false }
        let newValue = max(5, floor(containerWidth / charWidth))
        guard newValue != lineLength else { return false }
        lineLength = newValue
        return true
    }

    func heightForLine(charCount: Int) -> CGFloat {
        guard lineWrapping else { return lineHeight }
        let visualRows = visualRowsForLine(charCount: charCount)
        return CGFloat(visualRows) * lineHeight
    }

    func heightForGap(charCount: Int, logicalLineCount: Int) -> CGFloat {
        guard logicalLineCount > 0 else { return 0 }
        guard lineWrapping else { return CGFloat(logicalLineCount) * lineHeight }
        let lineCountValue = CGFloat(logicalLineCount)
        let chars = CGFloat(max(0, charCount))
        let baseLines = lineCountValue * lineLength * 0.5
        let extraRows = max(0, ceil((chars - baseLines) / lineLength))
        let totalRows = lineCountValue + extraRows
        return totalRows * lineHeight
    }

    private func visualRowsForLine(charCount: Int) -> Int {
        let chars = CGFloat(max(0, charCount))
        let extra = ceil((chars - lineLength) / max(1, lineLength - 5))
        return 1 + max(0, Int(extra))
    }
}
