import CoreGraphics
import Foundation

enum ComposerLayoutPreferences {
    static let widthKey = "muxy.composer.width"
    static let heightKey = "muxy.composer.height"
    static let expandedKey = "muxy.composer.expanded"

    static let defaultWidth: Double = 570
    static let defaultHeight: Double = 236
    static let defaultExpanded = false
    static let minWidth: CGFloat = 360
    static let minHeight: CGFloat = 180
    static let expandedWidth: CGFloat = 880
    static let expandedHeight: CGFloat = 620
    static let edgeMargin: CGFloat = 20
    static let centeredResizeGain: Double = 2

    static func size(width: Double, height: Double, isExpanded: Bool, available: CGSize) -> CGSize {
        let maximum = maximumSize(available: available)
        guard !isExpanded else {
            return CGSize(
                width: min(maximum.width, expandedWidth),
                height: min(maximum.height, expandedHeight)
            )
        }
        return CGSize(
            width: clamped(sanitized(width, fallback: defaultWidth), minimum: minWidth, maximum: maximum.width),
            height: clamped(sanitized(height, fallback: defaultHeight), minimum: minHeight, maximum: maximum.height)
        )
    }

    static func clampedWidth(_ width: CGFloat, available: CGSize) -> CGFloat {
        clamped(width, minimum: minWidth, maximum: maximumSize(available: available).width)
    }

    static func clampedHeight(_ height: CGFloat, available: CGSize) -> CGFloat {
        clamped(height, minimum: minHeight, maximum: maximumSize(available: available).height)
    }

    static func resizedLength(anchor: CGFloat, translation: CGFloat, isLeadingEdge: Bool) -> CGFloat {
        guard anchor.isFinite, translation.isFinite else { return anchor }
        let signed = isLeadingEdge ? -translation : translation
        return anchor + signed * CGFloat(centeredResizeGain)
    }

    static func maximumSize(available: CGSize) -> CGSize {
        CGSize(
            width: maximumLength(available: available.width, fallback: CGFloat(defaultWidth)),
            height: maximumLength(available: available.height, fallback: CGFloat(defaultHeight))
        )
    }

    private static func maximumLength(available: CGFloat, fallback: CGFloat) -> CGFloat {
        guard available.isFinite, available > 0 else { return fallback }
        return max(0, available - edgeMargin * 2)
    }

    private static func sanitized(_ value: Double, fallback: Double) -> CGFloat {
        guard value.isFinite, value > 0 else { return CGFloat(fallback) }
        return CGFloat(value)
    }

    private static func clamped(_ value: CGFloat, minimum: CGFloat, maximum: CGFloat) -> CGFloat {
        min(maximum, max(minimum, value))
    }
}
