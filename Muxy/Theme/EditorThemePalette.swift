import AppKit

struct EditorThemePalette {
    let background: NSColor
    let foreground: NSColor
    let accent: NSColor
    private let paletteColors: [Int: NSColor]

    @MainActor
    static var active: EditorThemePalette {
        let service = GhosttyService.shared
        let preview = ThemeService.shared.activeThemePreview(for: ThemeService.shared.activeAppearance())
        return resolve(
            preview: preview,
            fallbackBackground: service.backgroundColor,
            fallbackForeground: service.foregroundColor,
            fallbackAccent: service.accentColor,
            fallbackPaletteColor: { service.paletteColor(at: $0) }
        )
    }

    static func resolve(
        preview: ThemePreview?,
        fallbackBackground: NSColor,
        fallbackForeground: NSColor,
        fallbackAccent: NSColor,
        fallbackPaletteColor: (Int) -> NSColor?
    ) -> EditorThemePalette {
        let previewPalette = preview?.palette ?? []
        let paletteCount = max(16, previewPalette.count)
        var resolvedPalette: [Int: NSColor] = [:]
        for index in 0 ..< paletteCount {
            resolvedPalette[index] = previewPalette[safe: index] ?? fallbackPaletteColor(index)
        }
        let accent = previewPalette[safe: 4] ?? fallbackPaletteColor(4) ?? fallbackAccent
        return EditorThemePalette(
            background: preview?.background ?? fallbackBackground,
            foreground: preview?.foreground ?? fallbackForeground,
            accent: accent,
            paletteColors: resolvedPalette
        )
    }

    func paletteColor(at index: Int) -> NSColor? {
        guard index >= 0 else { return nil }
        return paletteColors[index]
    }
}

private extension Collection {
    subscript(safe index: Index) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
