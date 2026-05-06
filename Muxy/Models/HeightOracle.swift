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

    private func visualRowsForLine(charCount: Int) -> Int {
        let chars = CGFloat(max(0, charCount))
        return max(1, Int(ceil(chars / max(1, lineLength))))
    }
}
