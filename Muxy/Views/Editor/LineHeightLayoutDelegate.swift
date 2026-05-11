import AppKit

final class LineHeightLayoutDelegate: NSObject, NSLayoutManagerDelegate {
    var lineHeightMultiplier: CGFloat = 1.0
    var fallbackFont: NSFont

    init(fallbackFont: NSFont) {
        self.fallbackFont = fallbackFont
        super.init()
    }

    // swiftlint:disable:next function_parameter_count
    func layoutManager(
        _ layoutManager: NSLayoutManager,
        shouldSetLineFragmentRect lineFragmentRect: UnsafeMutablePointer<NSRect>,
        lineFragmentUsedRect: UnsafeMutablePointer<NSRect>,
        baselineOffset: UnsafeMutablePointer<CGFloat>,
        in textContainer: NSTextContainer,
        forGlyphRange glyphRange: NSRange
    ) -> Bool {
        guard lineHeightMultiplier > 1.0 + .ulpOfOne else { return false }

        let font = referenceFont(layoutManager: layoutManager, glyphRange: glyphRange)
        let ascent = font.ascender
        let descent = -font.descender
        let typographicHeight = ascent + descent
        let targetHeight = ceil(typographicHeight * lineHeightMultiplier)
        let paddingTop = (targetHeight - typographicHeight) / 2

        lineFragmentRect.pointee.size.height = targetHeight
        lineFragmentUsedRect.pointee.size.height = targetHeight
        baselineOffset.pointee = paddingTop + ascent
        return true
    }

    private func referenceFont(layoutManager: NSLayoutManager, glyphRange: NSRange) -> NSFont {
        if let storage = layoutManager.textStorage, glyphRange.length > 0 {
            let charIndex = layoutManager.characterIndexForGlyph(at: glyphRange.location)
            if charIndex < storage.length,
               let font = storage.attribute(.font, at: charIndex, effectiveRange: nil) as? NSFont
            {
                return font
            }
        }
        return fallbackFont
    }
}
