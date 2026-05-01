import AppKit
import Testing

@testable import Muxy

struct EditorThemePaletteTests {
    @Test("resolve prefers preview colors over Ghostty fallbacks")
    func resolvePrefersPreviewColors() {
        let previewBackground = NSColor(srgbRed: 0.1, green: 0.2, blue: 0.3, alpha: 1)
        let previewForeground = NSColor(srgbRed: 0.8, green: 0.7, blue: 0.6, alpha: 1)
        let previewPalette = (0 ..< 16).map { index in
            NSColor(srgbRed: CGFloat(index) / 20, green: 0.1, blue: 0.2, alpha: 1)
        }
        let preview = ThemePreview(
            name: "Dark Preview",
            background: previewBackground,
            foreground: previewForeground,
            palette: previewPalette
        )

        let resolved = EditorThemePalette.resolve(
            preview: preview,
            fallbackBackground: .white,
            fallbackForeground: .black,
            fallbackAccent: .red,
            fallbackPaletteColor: { _ in .green }
        )

        #expect(resolved.background == previewBackground)
        #expect(resolved.foreground == previewForeground)
        #expect(resolved.accent == previewPalette[4])
        #expect(resolved.paletteColor(at: 2) == previewPalette[2])
    }

    @Test("resolve fills incomplete preview palettes from fallbacks")
    func resolveFillsIncompletePreviewPalettesFromFallbacks() {
        let preview = ThemePreview(
            name: "Partial Preview",
            background: .black,
            foreground: .white,
            palette: [.red]
        )

        let resolved = EditorThemePalette.resolve(
            preview: preview,
            fallbackBackground: .white,
            fallbackForeground: .black,
            fallbackAccent: .blue,
            fallbackPaletteColor: { index in
                NSColor(srgbRed: CGFloat(index) / 20, green: 0.3, blue: 0.4, alpha: 1)
            }
        )

        #expect(resolved.paletteColor(at: 0) == .red)
        #expect(resolved.paletteColor(at: 3) == NSColor(srgbRed: 0.15, green: 0.3, blue: 0.4, alpha: 1))
        #expect(resolved.accent == NSColor(srgbRed: 0.2, green: 0.3, blue: 0.4, alpha: 1))
    }

    @Test("resolve uses fallback colors when no preview exists")
    func resolveUsesFallbackColorsWithoutPreview() {
        let fallbackBackground = NSColor(srgbRed: 0.9, green: 0.8, blue: 0.7, alpha: 1)
        let fallbackForeground = NSColor(srgbRed: 0.1, green: 0.2, blue: 0.3, alpha: 1)
        let fallbackAccent = NSColor(srgbRed: 0.4, green: 0.5, blue: 0.6, alpha: 1)

        let resolved = EditorThemePalette.resolve(
            preview: nil,
            fallbackBackground: fallbackBackground,
            fallbackForeground: fallbackForeground,
            fallbackAccent: fallbackAccent,
            fallbackPaletteColor: { index in
                index == 4 ? nil : NSColor(srgbRed: CGFloat(index) / 20, green: 0.4, blue: 0.5, alpha: 1)
            }
        )

        #expect(resolved.background == fallbackBackground)
        #expect(resolved.foreground == fallbackForeground)
        #expect(resolved.accent == fallbackAccent)
        #expect(resolved.paletteColor(at: 4) == nil)
        #expect(resolved.paletteColor(at: -1) == nil)
    }
}
