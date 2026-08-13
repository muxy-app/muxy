import CoreGraphics
import Foundation

struct TabStripLayout {
    static let minTabWidth: CGFloat = 44
    static let maxTabWidth: CGFloat = 200

    let perTabWidth: CGFloat
    let tabRowWidth: CGFloat
    let pinsNewTabButton: Bool

    init(availableWidth: CGFloat, tabCount: Int, maxTabWidth: CGFloat?, newTabButtonWidth: CGFloat) {
        let count = CGFloat(max(tabCount, 1))
        let isMeasured = availableWidth > 0
        let widthForTabs = max(availableWidth - newTabButtonWidth, 0)
        let idealWidth = isMeasured ? widthForTabs / count : Self.maxTabWidth
        let cappedWidth = maxTabWidth.map { min($0, idealWidth) } ?? idealWidth
        let overflows = isMeasured && idealWidth < Self.minTabWidth

        perTabWidth = max(Self.minTabWidth, cappedWidth)
        tabRowWidth = overflows ? widthForTabs : availableWidth
        pinsNewTabButton = overflows
    }
}
