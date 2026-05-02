import AppKit
import Testing

@testable import Muxy

@Suite("DiffRenderTheme")
@MainActor
struct DiffRenderThemeTests {
    @Test("current uses active editor palette foreground for default text color")
    func currentUsesActivePaletteForeground() {
        let preview = ThemePreview(
            name: "Preview",
            background: NSColor(calibratedRed: 0.1, green: 0.1, blue: 0.1, alpha: 1),
            foreground: NSColor(calibratedRed: 0.8, green: 0.7, blue: 0.6, alpha: 1),
            palette: []
        )
        let fallbackForeground = NSColor(calibratedRed: 0.2, green: 0.3, blue: 0.4, alpha: 1)

        let palette = EditorThemePalette.resolve(
            preview: preview,
            fallbackBackground: .black,
            fallbackForeground: fallbackForeground,
            fallbackAccent: .systemBlue,
            fallbackPaletteColor: { _ in nil }
        )

        #expect(palette.foreground.isEqual(preview.foreground))
        #expect(!palette.foreground.isEqual(fallbackForeground))
    }

    @Test("buildDiffAttributedString leaves plain text on the theme default color")
    func buildDiffAttributedStringUsesThemeDefaultColor() throws {
        let defaultColor = NSColor(calibratedRed: 0.7, green: 0.6, blue: 0.5, alpha: 1)
        let stringColor = NSColor(calibratedRed: 0.3, green: 0.8, blue: 0.4, alpha: 1)
        let regex = try NSRegularExpression(pattern: #"\"(?:\\.|[^\"\\])*\""#, options: [])
        let theme = DiffRenderTheme(
            rules: [DiffHighlightRule(regex: regex, color: stringColor)],
            additionColor: .systemGreen,
            deletionColor: .systemRed,
            defaultColor: defaultColor,
            additionBackground: .clear,
            deletionBackground: .clear,
            hunkBackground: .clear,
            collapsedBackground: .clear,
            font: DiffMetrics.font
        )

        let row = DiffDisplayRow(
            kind: .context,
            oldLineNumber: 1,
            newLineNumber: 1,
            oldText: nil,
            newText: "let value = 42",
            text: "let value = 42"
        )
        let rendered = buildDiffAttributedString(from: [row], theme: theme)
        let baseColor = try #require(
            rendered.attributedString.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor
        )

        #expect(baseColor.isEqual(defaultColor))
    }
}
